import {invoke} from "@tauri-apps/api";
import type {Encounter} from "$lib/types";

export const bosses = [
    "Dark Mountain Predator",
    "Destroyer Lucas",
    "Leader Lugaru",
    "Demon Beast Commander Valtan",
    "Ravaged Tyrant of Beasts",
    "Incubus Morphe",
    "Nightmarish Morphe",
    "Covetous Devourer Vykas",
    "Covetous Legion Commander Vykas",
    "Saydon",
    "Kakul",
    "Kakul-Saydon",
    "Encore-Desiring Kakul-Saydon",
    "Gehenna Helkasirs",
    "Prokel",
    "Prokel's Spiritual Echo",
    "Ashtarot",
    "Primordial Nightmare",
    "Brelshaza, Monarch of Nightmares",
    "Phantom Legion Commander Brelshaza",
    "Griefbringer Maurug",
    "Evolved Maurug",
    "Lord of Degradation Akkan",
    "Plague Legion Commander Akkan",
    "Lord of Kartheon Akkan",
    "Tienis",
    "Celestial Sentinel",
    "Prunya",
    "Lauriel",
    "Kaltaya, the Blooming Chaos",
    "Rakathus, the Lurking Arrogance",
    "Firehorn, Trampler of Earth",
    "Lazaram, the Trailblazer",
    "Gargadeth",
    "Sonavel",
    "Hanumatan",
    "Kungelanium",
    "Deskaluda"
];

export async function uploadLog(id: string, encounter: Encounter, settings: any, auto = false) {
    if (!bosses.includes(encounter.currentBossName)) {
        return 0;
    }

    if (auto) {
        if (settings.excludedCharacters.some((v: string) => encounter.entities[v])) {
            return 0;
        }

        if (encounter.entities[encounter.localPlayer]?.gearScore < Number(settings.minGearScore)) {
            return 0;
        }

        if (["Normal", "Hard", "Challenge"].includes(encounter.difficulty ?? "") && !settings.normal) {
            return 0;
        }

        if (["Trial", "Inferno"].includes(encounter.difficulty ?? "") && !settings.inferno) {
            return 0;
        }
    }

    const resp = await fetch(
        "https://logs.fau.dev/api/logs/upload" + (auto ? "?auto" : ""),
        {
            method: 'POST',
            headers: {'access_token': settings.accessToken},
            body: JSON.stringify(encounter)
        });
    if (!resp.ok && (resp.status == 500 || resp.status == 401)) {
        let error = "";
        if (resp.status == 500) {
            error = "server bwonk";
        } else if (resp.status == 401) {
            error = "invalid access token";
        }

        await invoke(
            "write_log",
            {message: "couldn't upload encounter " + id + " (" + encounter.currentBossName + ") - error: " + error}
        );
        return 0;
    }
    const body = await resp.json();
    if (body.error) {
        await invoke(
            "write_log",
            {message: "couldn't upload encounter " + id + " (" + encounter.currentBossName + ") - error: " + body.error.toLowerCase()}
        );
        await invoke("sync", {encounter: Number(id), upstream: 0, failed: true});
        return 0;
    }

    const upstream = body.id;
    await invoke("write_log", {message: "uploaded encounter " + id + " (" + encounter.currentBossName + ") upstream: " + upstream});
    await invoke("sync", {encounter: Number(id), upstream: upstream, failed: false});
    return upstream;
}