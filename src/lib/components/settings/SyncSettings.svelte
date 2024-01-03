<script lang="ts">
    import {settings} from "$lib/utils/settings";
    import SettingItem from "./SettingItem.svelte";
    import {onMount} from "svelte";
    import {invoke} from "@tauri-apps/api";
    import {uploadLog} from "$lib/utils/sync";

    let result = "";
    let checking = false;

    async function check() {
        if (checking) {
            return;
        }
        checking = true;

        await fetch("https://logs.fau.dev/api/users/@me", {
            headers: {access_token: $settings.sync.accessToken}
        }).then((resp) => {
            if (!resp.ok) {
                throw new Error("Invalid Access Token");
            }
            return resp.json();
        }).then((user) => {
            result = "Nice, " + (user.username || user.discordTag) + "!";
        }).catch(() => result = "Invalid Access Token :c");

        checking = false;
    }

    function saveExcludedCharacters() {
        $settings.sync.excludedCharacters = excludedCharacters.split("\n");
    }

    let excludedCharacters = "";
    onMount(() => {
        excludedCharacters = $settings.sync.excludedCharacters.join("\n");
    });

    let syncing = false;
    let synced = 0;
    let message = "";

    function syncPastLogs() {
        if (!$settings.sync.enabled) {
            message = "Sync is not enabled.";
            return;
        }

        if (!$settings.sync.auto) {
            message = "Auto upload is not enabled.";
            return;
        }

        if (!result.startsWith("Nice, ")) {
            message = "Check your access token before syncing past logs.";
            return;
        }

        if (syncing) {
            return;
        }
        syncing = true;
        synced = 0;

        (async () => {
            const ids = await invoke("get_sync_candidates", {});
            console.log(ids);

            for (let i = 0; i < ids.length; i++) {
                let id = ids[i];
                const encounter = await invoke("load_encounter", {id: id.toString()});
                let upstream = await uploadLog(id, encounter, $settings.sync, true);
                if (upstream != 0) {
                    synced++;
                }
                message = "Processing logs... (" + i + "/" + ids.length + ")";
            }
            console.log(synced);
            syncing = false;

            if (synced > 0) {
                message = "Uploaded " + synced + "logs.";
            } else {
                message = "No new logs were uploaded.";
            }
        })();
    }
</script>

<div class="flex flex-col space-y-4 divide-y-[1px]">
    <div class="mt-4 flex flex-col space-y-2 px-2">
        <SettingItem
                name="Sync (logs.fau.dev)"
                description="Enable log uploads"
                bind:setting={$settings.sync.enabled}/>
        <p>Access Token</p>
        <input
                type="password"
                bind:value={$settings.sync.accessToken}
                class="focus:border-accent-500 block w-80 rounded-lg border border-gray-600 bg-zinc-700 text-xs text-zinc-300 placeholder-gray-400 focus:ring-0"
                placeholder="owo owo owo"/>
        <div>
            <button on:click={check}
                    class="rounded-md inline text-xs bg-zinc-600 mr-0.5 py-1 px-1.5 w-fit hover:bg-zinc-700">
                Check
            </button>
            <span>{result}</span>
        </div>
    </div>
    <div class="pt-4 mt-4 flex flex-col space-y-2 px-2">
        <SettingItem
                name="Auto Upload"
                description="Upload logs on clear"
                bind:setting={$settings.sync.auto}/>
        <p>Excluded Characters</p>
        <div>
        <textarea bind:value={excludedCharacters}
                  on:input={saveExcludedCharacters}
                  class="focus:ring-accent-500 focus:border-accent-50 rounded-lg border border-gray-600 bg-gray-700 text-sm text-white"/>
        </div>
        <p>Raids</p>
        <SettingItem
                name="Legion Raids / Abyssal Dungeons / Guardian Raids"
                bind:setting={$settings.sync.normal}/>
        <div class="ml-10">
            <p class="text-xs mb-2">Minimum Gear Score</p>
            <select
                    bind:value={$settings.sync.minGearScore}
                    class="focus:ring-accent-500 focus:border-accent-500 yx-2 block w-28 rounded-lg border border-gray-600 bg-gray-700 py-1 text-sm text-white placeholder-gray-400">
                {#each ["1415", "1490", "1520", "1540", "1560", "1580", "1600", "1620"] as gearScore}
                    <option value={gearScore} selected>{gearScore}</option>
                {/each}
            </select>
        </div>
        <SettingItem
                name="Inferno Raids"
                bind:setting={$settings.sync.inferno}/>
    </div>
    <div class="pt-4 mt-4 flex flex-col space-y-2 px-2">
        <div class="flex items-center space-x-2">
            <div>Sync Past Logs:</div>
            {#if !syncing}
                <button class="rounded-md bg-zinc-600 p-1 hover:bg-zinc-700" on:click={syncPastLogs}>Sync</button>
            {:else}
                <button class="rounded-md bg-zinc-600 p-1 hover:bg-zinc-700" disabled>Syncing...</button>
            {/if}
        </div>
        {#if message}
            <p>{message}</p>
        {/if}
    </div>
</div>