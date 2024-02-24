import { invoke } from "@tauri-apps/api";
import type { Encounter } from "$lib/types";

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
    "Deskaluda",
    "Caliligos",
    "Achates"
];

export async function uploadLog(id: string, encounter: Encounter, settings: any, method = "auto") {
    if (!bosses.includes(encounter.currentBossName)) {
        return 0;
    }

    if (method === "auto" || method == "bulk") {
        if (encounter.difficulty === "Challenge") {
            return 0;
        }

        if (encounter.currentBossName === "Achates" && encounter.difficulty === "Normal") {
            return 0;
        }

        if (settings.excludedCharacters.some((v: string) => encounter.entities[v])) {
            return 0;
        }

        if (
            ["Normal", "Hard", "Extreme"].includes(encounter.difficulty ?? "") &&
            (encounter.entities[encounter.localPlayer]?.gearScore < Number(settings.minGearScore) || !settings.normal)
        ) {
            return 0;
        }

        if (["Trial", "Inferno"].includes(encounter.difficulty ?? "") && !settings.inferno) {
            return 0;
        }

        if (encounter.difficulty === "" && !settings.unclassified) {
            return 0;
        }
    }

    const resp = await fetch("https://logs.fau.dev/api/logs/upload?" + method, {
        method: "POST",
        headers: { access_token: settings.accessToken },
        body: JSON.stringify(encounter)
    });
    if (!resp.ok && (resp.status == 500 || resp.status == 401)) {
        let error = "";
        if (resp.status == 500) {
            error = "server bwonk";
        } else if (resp.status == 401) {
            error = "invalid access token";
        }

        await invoke("write_log", {
            message: "couldn't upload encounter " + id + " (" + encounter.currentBossName + ") - error: " + error
        });
        return 0;
    }
    const body = await resp.json();
    if (body.error) {
        await invoke("write_log", {
            message:
                "couldn't upload encounter " +
                id +
                " (" +
                encounter.currentBossName +
                ") - error: " +
                body.error.toLowerCase()
        });
        await invoke("sync", { encounter: Number(id), upstream: 0, failed: true });
        return 0;
    }

    const upstream = body.id;
    await invoke("write_log", {
        message: "uploaded encounter " + id + " (" + encounter.currentBossName + ") upstream: " + upstream
    });
    await invoke("sync", { encounter: Number(id), upstream: upstream, failed: false });
    return upstream;
}
