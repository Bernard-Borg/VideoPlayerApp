<script setup lang="ts">
import { ref, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useIntervalFn, useLocalStorage, useOnline } from "@vueuse/core";
import { X, Search, ExternalLink } from "lucide-vue-next";
import { Trash2 } from "lucide-vue-next";
import { useNotification, useWindowClose } from "../composables";
import type { Schema$SearchListResponse, Schema$SearchResult } from "youtube-api";
import type { History } from "../types";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

const search = ref<string>("");
const loadingText = ref<string>("");
const searchFailed = ref<boolean>(false);
const searchResults = ref<Array<Schema$SearchResult>>([]);
const failureReason = ref<string>("");

const preferredQuality = useLocalStorage<string>("preferred-quality", "1");

const history = useLocalStorage<History>("history", {
    volume: 0.5,
    time: 0,
    title: undefined,
    isYoutube: false,
    video: undefined,
    youtubeCode: undefined
});

const online = useOnline();
const { add } = useNotification();

const { closeWindow } = useWindowClose("youtube", true);

// Loading... animation
const animateLoadingText = () => {
    if (loadingText.value.includes("...")) {
        loadingText.value = "Loading";
    } else if (loadingText.value.includes("..")) {
        loadingText.value = "Loading...";
    } else if (loadingText.value.includes(".")) {
        loadingText.value = "Loading..";
    } else {
        loadingText.value = "Loading.";
    }
};

const { pause, resume } = useIntervalFn(
    () => {
        animateLoadingText();
    },
    250,
    { immediate: false }
);

const searchVideo = async () => {
    const result = (await fetch(`https://videovish.vercel.app/api/videovish?search=${search.value}&region=US`, {
        method: "POST",
        body: JSON.stringify({
            apiKey: import.meta.env.VITE_API_KEY
        })
    }).then((result) => result.json())) as Schema$SearchListResponse;

    searchResults.value = result.items ?? [];
    getCurrentWebviewWindow().setFullscreen(true);
};

const startVideoDownload = (url: string, code: string) => {
    resume();

    invoke("download_video", {
        url: url,
        code: code,
        quality: preferredQuality.value ?? "1"
    }).then((result) => {
        if (result === "") {
            closeWindow(); //if download was successful, close window
        } else {
            // if download wasn't, display error
            pause();
            loadingText.value = "";
            searchFailed.value = true;
            failureReason.value = result as string;
        }
    });
};

const downloadVideo = async () => {
    searchResults.value = [];
    searchFailed.value = false;
    failureReason.value = "";

    // Prevents user from clicking enter multiple times
    if (loadingText.value) {
        return;
    }

    // Cannot download video if offline
    if (!online.value) {
        searchFailed.value = true;
        failureReason.value = "Cannot play YouTube video while offline";
        return;
    }

    const regExp = /^.*((youtu.be\/)|(v\/)|(\/u\/\w\/)|(embed\/)|(watch\?))\??v?=?([^#&?]*).*/;
    const match = search.value.match(regExp);
    const youtubeVideoCode = match && match[7].length === 11 ? match[7] : false;

    if (!youtubeVideoCode) {
        await searchVideo();
        return;
    }

    startVideoDownload(search.value, youtubeVideoCode);
};

const getVideo = (event: KeyboardEvent) => {
    if (event.key === "Enter") {
        downloadVideo();
    }
};

watch(search, () => {
    searchFailed.value = false;
    failureReason.value = "";
});

onUnmounted(() => {
    pause();
});

const formatBytes = (bytes: number, decimals: number = 2) => {
    if (!+bytes) return "0 Bytes";

    const k = 1024;
    const dm = decimals < 0 ? 0 : decimals;
    const sizes = ["Bytes", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB", "YiB"];

    const i = Math.floor(Math.log(bytes) / Math.log(k));

    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(dm))} ${sizes[i]}`;
};

const clearCache = async () => {
    await invoke("clear_cache").then((result) => {
        if (result === "EMPTY") {
            add({
                text: "Cache already empty",
                type: "info",
                timeout: 3000
            });
        } else if (result) {
            const numberResult = parseInt(result as string);

            if (isNaN(numberResult)) {
                console.error("Clear cache return is not a number");
                return;
            }

            result = formatBytes(numberResult);

            add({
                text: `Cleared ${result} from cache`,
                type: "success",
                timeout: 3000
            });

            if (history.value.isYoutube) {
                history.value.video = undefined;
                history.value.time = 0;
                history.value.title = undefined;
                history.value.isYoutube = false;
                history.value.youtubeCode = undefined;
            }
        }
    });
};
</script>

<template>
    <div class="p-2 bg-charcoal rounded-md w-full h-full">
        <div class="h-full px-12 py-10 cursor-grab flex flex-col" data-tauri-drag-region>
            <h1 class="font-bold text-3xl pointer-events-none">Search video</h1>
            <X color="white" class="absolute top-[10px] right-[10px]" @click="closeWindow" />
            <!-- Quality select -->
            <div class="flex items-center mt-4 justify-end w-[600px]">
                <span class="inline-block mr-3 pointer-events-none">Preferred Quality</span>
                <select v-model="preferredQuality" class="cursor-pointer">
                    <option value="1">1080p</option>
                    <option value="2">720p</option>
                    <option value="3">480p</option>
                    <option value="4">240p</option>
                    <option value="5">Audio only</option>
                </select>
            </div>
            <!-- Search bar -->
            <div class="flex mt-3 gap-5 items-center">
                <div class="flex flex-col w-[600px]">
                    <div class="flex items-center">
                        <input
                            v-model="search"
                            ref="youtubeSearch"
                            :class="`rounded-md p-[15px] w-full text-md ${
                                failureReason ? 'outline outline-1 outline-red-600' : ''
                            }`"
                            placeholder="e.g. PewDiePie or https://www.youtube.com/watch?v=dQw4w9WgXcQ"
                            @keydown.tab="search = 'https://www.youtube.com/watch?v=dQw4w9WgXcQ'"
                            @keyup="getVideo"
                        />
                        <Search @click="downloadVideo" color="#252526" class="-ml-[40px] cursor-pointer z-50" />
                    </div>
                    <span class="text-red-600 text-sm self-end">{{ failureReason }}</span>
                </div>
                <button
                    class="outline outline-1 outline-white text-white p-1 rounded-md aspect-square h-[35px] flex justify-center items-center hover:bg-red-500"
                    title="Clear Cache"
                    @click="clearCache"
                >
                    <Trash2 />
                </button>
            </div>
            <span class="absolute left-0 bottom-0 p-3 pointer-events-none" v-if="loadingText">{{ loadingText }}</span>
            <!-- Search results -->
            <div class="flex flex-col gap-3 text-white mt-10 overflow-y-scroll overflow-x-clip">
                <div
                    v-for="searchResult in searchResults"
                    class="flex items-center px-6 py-0 border border-1 border-black justify-between bg-red-900 hover:bg-red-800 rounded-md cursor-pointer select-none"
                    title="Open in YouTube"
                    @click.self="
                        startVideoDownload(
                            `https://www.youtube.com/watch?v=${searchResult.id?.videoId}`,
                            searchResult.id?.videoId ?? ''
                        )
                    "
                >
                    <template v-if="searchResult.id?.videoId">
                        <div class="flex flex-col pointer-events-none">
                            <span class="text-lg font-bold">{{ searchResult.snippet?.title }}</span>
                            <span
                                ><span class="font-semibold">Channel: </span
                                >{{ searchResult.snippet?.channelTitle }}</span
                            >
                        </div>
                        <div class="flex gap-10">
                            <div class="flex items-center gap-3">
                                <a
                                    v-if="searchResult.id?.videoId"
                                    :href="`https://www.youtube.com/watch?v=${searchResult.id?.videoId}`"
                                    target="_blank"
                                >
                                    <ExternalLink :size="35" title="Open in YouTube" />
                                </a>
                            </div>
                            <img
                                v-if="searchResult.snippet?.thumbnails?.maxres?.url"
                                :src="searchResult.snippet.thumbnails.maxres.url"
                                class="aspect-video h-[140px] overflow-clip"
                            />
                            <img
                                v-else-if="searchResult.snippet?.thumbnails?.high?.url"
                                :src="searchResult.snippet.thumbnails.high.url"
                                class="aspect-video h-[140px] overflow-clip"
                            />
                            <img
                                v-else-if="searchResult.snippet?.thumbnails?.medium?.url"
                                :src="searchResult.snippet.thumbnails.medium.url"
                                class="aspect-video h-[140px] overflow-clip"
                            />
                            <img
                                v-else-if="searchResult.snippet?.thumbnails?.standard?.url"
                                :src="searchResult.snippet.thumbnails.standard.url"
                                class="aspect-video h-[140px] overflow-clip"
                            />
                        </div>
                    </template>
                </div>
            </div>
        </div>
    </div>
</template>
