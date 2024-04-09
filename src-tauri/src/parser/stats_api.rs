use crate::parser::debug_print;
use crate::parser::encounter_state::EncounterState;
use crate::parser::entity_tracker::{Entity, EntityTracker};
use async_recursion::async_recursion;
use chrono::{DateTime, Utc};
use hashbrown::{HashMap, HashSet};
use log::{info, warn};
use md5::compute;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Window, Wry};

const API_URL: &str = "https://inspect.fau.dev/query";

pub struct StatsApi {
    cache: Arc<Mutex<HashMap<String, PlayerStats>>>,
    stats_cache: Arc<Mutex<HashMap<String, Stats>>>,
    cache_status: Arc<AtomicBool>,
    cancellation_flag: Arc<AtomicBool>,
    client: Arc<Client>,
    hash_cache: Arc<Mutex<HashMap<String, String>>>,
    window: Arc<Window<Wry>>,
}

impl StatsApi {
    pub fn new(window: Window<Wry>) -> Self {
        Self {
            window: Arc::new(window),
            cache: Arc::new(Mutex::new(HashMap::new())),
            stats_cache: Arc::new(Mutex::new(HashMap::new())),
            cache_status: Arc::new(AtomicBool::new(false)),
            cancellation_flag: Arc::new(AtomicBool::new(false)),
            client: Arc::new(Client::new()),
            hash_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn sync(
        &mut self,
        party: Vec<Vec<String>>,
        state: &EncounterState,
        entity_tracker: &EntityTracker,
        cached: &HashMap<u64, String>,
    ) {
        let region = match state.region.as_ref() {
            Some(region) => region.clone(),
            None => cached.get(&0).cloned().unwrap_or_default(),
        };
        if party.is_empty() || region.is_empty() {
            debug_print(format_args!("party info is empty or region is not set"));
            self.broadcast("missing_info");
            return;
        }
        if state.raid_difficulty.is_empty()
            || state.raid_difficulty == "Inferno"
            || state.raid_difficulty == "Challenge"
        {
            debug_print(format_args!("stats not valid in current zone"));
            self.broadcast("invalid_zone");
            return;
        }

        let player_names = party.iter().flatten().cloned().collect::<HashSet<String>>();
        if let (Ok(mut cache), Ok(mut stats_cache), Ok(mut hash_cache)) = (
            self.cache.lock(),
            self.stats_cache.lock(),
            self.hash_cache.lock(),
        ) {
            cache.retain(|player, _| player_names.contains(player));
            stats_cache.retain(|player, _| player_names.contains(player));
            hash_cache.retain(|player, _| player_names.contains(player));
        }
        let mut player_hashes: Vec<PlayerHash> = Vec::new();
        if let Ok(hash_cache) = self.hash_cache.lock() {
            for player in player_names.iter() {
                let entity_id = match state.encounter.entities.get(player) {
                    Some(entity) => entity.id,
                    None => continue,
                };
                if let Some(entity) = entity_tracker.entities.get(&entity_id) {
                    if let Some(hash) = self.get_hash(entity) {
                        if hash_cache
                            .get(player)
                            .map_or(false, |cached_hash| cached_hash == &hash)
                        {
                            continue;
                        } else {
                            player_hashes.push(PlayerHash {
                                name: player.clone(),
                                hash,
                            });
                        }
                    }
                }
            }
        }
        if player_hashes.is_empty() {
            return;
        }

        self.request(region, player_hashes);
    }

    fn request(&mut self, region: String, players: Vec<PlayerHash>) {
        let client_clone = Arc::clone(&self.client);
        let cache_clone = Arc::clone(&self.cache);
        let cache_status = Arc::clone(&self.cache_status);
        let stats_cache_clone = Arc::clone(&self.stats_cache);
        let hash_cache_clone = Arc::clone(&self.hash_cache);

        self.cancellation_flag.store(true, Ordering::SeqCst);
        let new_cancellation_flag = Arc::new(AtomicBool::new(false));
        self.cancellation_flag = Arc::clone(&new_cancellation_flag);

        self.broadcast("requesting_stats");
        let window_clone = Arc::clone(&self.window);
        tokio::task::spawn(async move {
            make_request(
                &client_clone,
                &window_clone,
                &region,
                cache_clone,
                stats_cache_clone,
                hash_cache_clone,
                cache_status,
                new_cancellation_flag,
                players,
                0,
            )
            .await;
        });
    }

    pub fn get_hash(&self, player: &Entity) -> Option<String> {
        if player.items.equip_list.is_none()
            || player.character_id == 0
            || player.class_id == 0
            || player.name == "You"
            || !player
                .name
                .chars()
                .next()
                .unwrap_or_default()
                .is_uppercase()
        {
            return None;
        }

        let mut equip_data: [u32; 32] = [0; 32];
        if let Some(equip_list) = player.items.equip_list.as_ref() {
            for item in equip_list.iter() {
                if item.slot >= 32 {
                    continue;
                }
                equip_data[item.slot as usize] = item.id;
            }
        }

        let data = format!(
            "{}{}{}{}",
            player.name,
            player.class_id,
            player.character_id,
            equip_data.iter().map(|x| x.to_string()).collect::<String>()
        );

        Some(format!("{:x}", compute(data)))
    }

    pub fn get_all_stats(&self, difficulty: &str) -> Option<HashMap<String, PlayerStats>> {
        if difficulty.is_empty() || difficulty == "Inferno" || difficulty == "Challenge" {
            return None;
        }
        if self.cache_status.load(Ordering::Relaxed) {
            if let Ok(cache) = self.cache.lock() {
                Some(cache.clone())
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn get_stats(&self, difficulty: &str) -> Option<HashMap<String, Stats>> {
        if difficulty.is_empty() || difficulty == "Inferno" || difficulty == "Challenge" {
            return None;
        }
        if self.cache_status.load(Ordering::Relaxed) {
            if let Ok(cache) = self.stats_cache.lock() {
                Some(cache.clone())
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn broadcast(&self, message: &str) {
        self.window
            .emit("rdps", message)
            .expect("failed to emit rdps message");
    }
}

#[async_recursion]
async fn make_request(
    client: &Client,
    window: &Arc<Window<Wry>>,
    region: &str,
    cache: Arc<Mutex<HashMap<String, PlayerStats>>>,
    stats_cache: Arc<Mutex<HashMap<String, Stats>>>,
    hash_cache: Arc<Mutex<HashMap<String, String>>>,
    cache_status: Arc<AtomicBool>,
    cancellation: Arc<AtomicBool>,
    players: Vec<PlayerHash>,
    current_retries: usize,
) {
    if current_retries > 24 {
        warn!("# of retries exceeded, failed to fetch player stats for {:?}", players);
        window
            .emit("rdps", "request_failed")
            .expect("failed to emit rdps message");
        return;
    }

    let request_body = json!({
        "region": region.clone(),
        "characters": players.clone(),
    });
    debug_print(format_args!("requesting player stats"));
    // debug_print(format_args!("{:?}", players));
    // println!("{:?}", request_body);

    match client.post(API_URL).json(&request_body).send().await {
        Ok(response) => match response.json::<HashMap<String, PlayerStats>>().await {
            Ok(data) => {
                let mut missing_players: Vec<PlayerHash> = Vec::new();
                if let (Ok(mut cache_clone), Ok(mut stats_cache_clone), Ok(mut hash_cache_clone)) =
                    (cache.lock(), stats_cache.lock(), hash_cache.lock())
                {
                    missing_players = players
                        .iter()
                        .filter(|player| !data.contains_key(&player.name))
                        .cloned()
                        .collect();

                    for (name, stats) in data {
                        hash_cache_clone.insert(
                            name.clone(),
                            players
                                .iter()
                                .find(|p| p.name == name)
                                .unwrap()
                                .hash
                                .clone(),
                        );
                        stats_cache_clone.insert(
                            name.clone(),
                            Stats {
                                crit: stats.stats.get(&0).cloned().unwrap_or_default(),
                                spec: stats.stats.get(&1).cloned().unwrap_or_default(),
                                atk_power: stats.stats.get(&4).cloned().unwrap_or_default(),
                                add_dmg: stats.stats.get(&5).cloned().unwrap_or_default(),
                            },
                        );
                        cache_clone.insert(name, stats);
                    }
                    // debug_print(format_args!("{:?}", stats_cache_clone));
                }

                if missing_players.is_empty() {
                    debug_print(format_args!("received player stats"));
                    cache_status.store(true, Ordering::Relaxed);
                    window
                        .emit("rdps", "request_success")
                        .expect("failed to emit rdps message");
                } else {
                    cache_status.store(false, Ordering::Relaxed);
                    if cancellation.load(Ordering::SeqCst) {
                        debug_print(format_args!("request cancelled"));
                        return;
                    }
                    tokio::time::sleep(core::time::Duration::from_secs(2)).await;
                    if cancellation.load(Ordering::SeqCst) {
                        debug_print(format_args!("request cancelled"));
                        return;
                    }
                    debug_print(format_args!(
                        "missing stats for: {:?}, retrying, attempt {}",
                        missing_players,
                        current_retries + 1
                    ));
                    // retry request with missing players
                    // until we receive stats for all players
                    make_request(
                        client,
                        window,
                        region,
                        Arc::clone(&cache),
                        Arc::clone(&stats_cache),
                        Arc::clone(&hash_cache),
                        cache_status,
                        cancellation,
                        missing_players,
                        current_retries + 1,
                    )
                    .await;
                }
            }
            Err(e) => {
                cache_status.store(false, Ordering::Relaxed);
                warn!("failed to parse player stats: {:?}", e);
            }
        },
        Err(e) => {
            cache_status.store(false, Ordering::Relaxed);
            warn!("failed to fetch player stats: {:?}", e);
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub crit: u32,
    pub spec: u32,
    pub atk_power: u32,
    pub add_dmg: u32,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayerStats {
    pub name: String,
    pub stats: HashMap<u8, u32>,
    pub elixirs: Option<Vec<ElixirData>>,
    pub gems: Option<Vec<GemData>>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ElixirData {
    pub slot: u8,
    pub entries: Vec<ElixirEntry>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ElixirEntry {
    pub id: u32,
    pub level: u8,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GemData {
    pub id: u32,
    pub skill_id: u32,
    #[serde(alias = "type")]
    pub gem_type: u8,
    pub value: u32,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EngravingData {
    pub name: String,
    pub level: u8,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayerHash {
    pub name: String,
    pub hash: String,
}
