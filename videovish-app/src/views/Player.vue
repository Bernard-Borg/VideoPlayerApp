<script setup lang="ts">
import NotificationRenderer from "../NotificationRenderer.vue";
import VideoChooser from "./VideoChooser.vue";
import { onBeforeMount, onMounted, onUnmounted, ref, computed, watch, shallowRef } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { getMatches } from "@tauri-apps/plugin-cli";
import { open, save } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { exists } from "@tauri-apps/plugin-fs";
import { basename, downloadDir, extname, join } from "@tauri-apps/api/path";
import { getCurrentWebviewWindow, getAllWebviewWindows, WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useDraggable, useLocalStorage, useMediaControls, useThrottleFn, useTimeoutFn } from "@vueuse/core";
import {
    Info,
    Play,
    Pause,
    Repeat,
    Rewind,
    FastForward,
    Maximize,
    Minimize,
    Youtube,
    VolumeX,
    Volume1,
    Volume2,
    Minus,
    Square,
    X,
    Home,
    Save
} from "lucide-vue-next";
import { useNotification } from "../composables";
import type { History } from "../types";
const appWindow = getCurrentWebviewWindow();

const NUM_KEYS = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
const PLAYBACK_SPEEDS = [0.07, 0.1, 0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 2.5, 3, 5, 7.5, 10, 12, 14, 16];
const VALID_EXTENSIONS = ["ogg", "webm", "mp4", "mkv", "mov", "mp3"];

// Close all windows when the main one is closed
appWindow.onCloseRequested(() => {
    getAllWebviewWindows().then((windows) =>
        windows.forEach((window) => {
            window.close();
        })
    );
});

const youtubeButton = ref<HTMLButtonElement | null>(null);
const progressBar = ref<HTMLDivElement | null>(null);
const videoPlayer = shallowRef<HTMLVideoElement | null>(null);
const progressCircle = shallowRef<HTMLDivElement | null>(null);

const looping = ref<boolean>();
const isFullscreen = ref<boolean>(false);
const uiHidden = ref<boolean>(false);

const volumeCache = ref<number>(0.5);

const videoSrc = ref<string>("");
const videoTitle = ref<string>();
const choosingVideo = ref<boolean>(false);

const transformationIcon = ref();
const transformationText = ref<string>();

const circlePosition = computed(() => (currentTime.value / duration.value) * 100);

const { playing, currentTime, duration, volume, rate } = useMediaControls(videoPlayer, {
    src: videoSrc
});

const { add } = useNotification();

useDraggable(progressCircle, {
    axis: "x",
    onMove: (e) => {
        if (progressBar.value) {
            seekVideoSection((e.x - progressBar.value.getBoundingClientRect().x) / progressBar.value.clientWidth);
        }
    },
    onEnd: (e) => {
        if (progressBar.value) {
            seekVideoSection((e.x - progressBar.value.getBoundingClientRect().x) / progressBar.value.clientWidth);
        }
    }
});

let playbackIndex = 5;

const toHHMMSS = (time: string): string => {
    const sec_num = parseInt(time, 10);

    const hours = Math.floor(sec_num / 3600);
    const minutes = Math.floor((sec_num - hours * 3600) / 60);
    const seconds = sec_num - hours * 3600 - minutes * 60;

    return `${hours < 10 ? `0${hours}` : hours}:${minutes < 10 ? `0${minutes}` : minutes}:${
        seconds < 10 ? `0${seconds}` : seconds
    }`;
};

const { start, stop } = useTimeoutFn(() => {
    uiHidden.value = true;
}, 1250);

// A transformation alert shows up in the top right of the screen when the user
// does certain interactions (like increasing volumew)
const displayTransformationAlert = (icon: any, text?: string) => {
    transformationIcon.value = icon;

    if (text) {
        transformationText.value = text;
    } else {
        transformationText.value = "";
    }

    useTimeoutFn(() => {
        transformationIcon.value = undefined;
        transformationText.value = "";
    }, 600);
};

const getVolumeIcon = (volume: number) => {
    let icon;

    if (volume == 0) {
        icon = VolumeX;
    } else if (volume < 0.5) {
        icon = Volume1;
    } else {
        icon = Volume2;
    }

    return icon;
};

const increaseVolume = () => {
    if (volume.value < 1) {
        volume.value = parseFloat((volume.value + 0.1).toFixed(1));
    }

    displayTransformationAlert(getVolumeIcon(volume.value), `${parseFloat(volume.value.toFixed(1)) * 100}%`);
};

const decreaseVolume = () => {
    if (volume.value > 0) {
        volume.value = parseFloat((volume.value - 0.1).toFixed(1));
    }

    displayTransformationAlert(getVolumeIcon(volume.value), `${parseFloat(volume.value.toFixed(1)) * 100}%`);
};

const mute = () => {
    volumeCache.value = volume.value;

    volume.value = 0;
    displayTransformationAlert(getVolumeIcon(volume.value), `${parseFloat(volume.value.toFixed(1)) * 100}%`);
};

const unmute = () => {
    volume.value = volumeCache.value;
    displayTransformationAlert(getVolumeIcon(volume.value), `${parseFloat(volume.value.toFixed(1)) * 100}%`);
};

const forwardVideo = (amount: number) => {
    if (currentTime.value <= duration.value - amount) {
        currentTime.value += amount;
    }

    displayTransformationAlert(FastForward, `${amount}s`);
};

const rewindVideo = (amount: number) => {
    if (currentTime.value >= amount) {
        currentTime.value -= amount;
    } else {
        currentTime.value = 0;
    }

    displayTransformationAlert(Rewind, `${amount}s`);
};

const seekVideoSection = (videoSection: number) => {
    currentTime.value = videoSection * duration.value;
};

const setFullscreen = async () => {
    isFullscreen.value = !isFullscreen.value;

    await appWindow.setFullscreen(isFullscreen.value);
};

const playVideo = async () => {
    if (!videoPlayer.value) {
        return;
    }

    if (playing.value) {
        videoPlayer.value.pause();
        displayTransformationAlert(Pause);
    } else {
        try {
            await videoPlayer.value.play();
        } catch (err) {}

        displayTransformationAlert(Play);
    }
};

// Changes the playing speed of the video up or down depending on pre-set playback speeds
const changePlayrate = (direction: number) => {
    if (direction == 1) {
        if (playbackIndex < PLAYBACK_SPEEDS.length - 1) {
            playbackIndex++;
        }
    } else {
        if (playbackIndex > 0) {
            playbackIndex--;
        }
    }

    rate.value = PLAYBACK_SPEEDS[playbackIndex];

    stop();
    uiHidden.value = false;
    start();
};

// Loops the video
const loopBack = () => {
    if (videoPlayer.value) {
        videoPlayer.value.currentTime = 0;
        videoPlayer.value.play();
    }
};

// Sets the src of the video
const setVideoSource = async (filepath: string, title?: string | null) => {
    if (!title) {
        title = await basename(filepath);
        const extension = await extname(filepath);
        title = title.replace(`.${extension}`, "");
    }

    history.value.video = filepath;
    choosingVideo.value = false;

    filepath = convertFileSrc(filepath);

    videoTitle.value = title;
    videoSrc.value = filepath;

    let fileExtension = await extname(filepath);

    if (!VALID_EXTENSIONS.includes(fileExtension.toLowerCase())) {
        console.warn("WARNING: file type might not work properly");
    }

    playVideo();
};

// Shows the file dialog to pick a video to play
const showVideoDialog = async () => {
    await open({
        multiple: false,
        filters: [{ name: "Supported Video Files", extensions: VALID_EXTENSIONS }]
    }).then((videoPath) => {
        if (videoPath) {
            history.value.isYoutube = false;
            setVideoSource(videoPath as string);
        }
    });
};

// Shows the help modal
const showHelpWindow = async () => {
    if (playing.value) {
        playVideo();
    }

    const helpWindow = await WebviewWindow.getByLabel("help");

    if (helpWindow) {
        helpWindow.close();
    }

    await invoke("show_help_window");
};

// Shows the youtube video picker modal
const showYoutubeModal = async () => {
    //The following is to prevent the modal from being opened again when pressing space after it has been closed
    youtubeButton.value?.blur();

    if (playing.value) {
        playVideo();
    }

    await invoke("show_youtube_modal");
};

const saveYouTubeVideo = async () => {
    const lastYoutubeCode = history.value.youtubeCode;

    if (!lastYoutubeCode) {
        add({
            text: "You haven't downloaded a YouTube video yet",
            type: "warning",
            timeout: 5000
        });
    }

    const downloadDirectory = await downloadDir();

    const filePath = await save({
        title: "Save YouTube video",
        defaultPath: await join(downloadDirectory, `${videoTitle.value ?? "video"}.mp4`),
        filters: [{ name: "Video", extensions: VALID_EXTENSIONS }]
    });

    if (!filePath) {
        return;
    }

    invoke("save_youtube_video", { code: lastYoutubeCode, pathToSave: filePath }).then((result) => {
        if (result) {
            add({
                text: result as string,
                type: "error",
                timeout: 10000
            });
        } else {
            add({
                text: "Saved successfully",
                type: "success",
                timeout: 3000
            });
        }
    });
};

const setLoopMode = () => {
    if (looping.value) {
        videoPlayer.value?.removeEventListener("ended", loopBack);
    } else {
        videoPlayer.value?.addEventListener("ended", loopBack);
    }

    looping.value = !looping.value;
};

const keyDownEventHandler = (event: KeyboardEvent) => {
    if (event.key.toLowerCase() == "d") {
        changePlayrate(1);
    } else if (event.key.toLowerCase() == "s") {
        changePlayrate(0);
    } else if (event.key.toLowerCase() == "j") {
        rewindVideo(10);
    } else if (event.key.toLowerCase() == "k") {
        forwardVideo(10);
    } else if (event.key.toLowerCase() == "m") {
        if (volume.value) {
            mute();
        } else {
            unmute();
        }
    } else if (event.key == "F1") {
        showHelpWindow();
    } else if (event.key == "F11") {
        setFullscreen();
    } else if (event.key == "ArrowLeft") {
        rewindVideo(5);
    } else if (event.key == "ArrowRight") {
        forwardVideo(5);
    } else if (event.key == "ArrowUp") {
        increaseVolume();
    } else if (event.key == "ArrowDown") {
        decreaseVolume();
    } else if (NUM_KEYS.includes(event.key)) {
        seekVideoSection(parseInt(event.key) / 10);
    }
};

const keyUpEventHandler = (event: KeyboardEvent) => {
    if (event.key == " ") {
        playVideo();
    } else if (event.key.toLowerCase() == "o" && event.ctrlKey) {
        showVideoDialog();
    } else if (event.key == "/" && event.ctrlKey) {
        showHelpWindow();
    }
};

const mouseMoveHandler = () => {
    if (choosingVideo.value) {
        uiHidden.value = true;
        return;
    }

    stop();
    uiHidden.value = false;
    start();
};

const wheelHandler = useThrottleFn((e: WheelEvent) => {
    if (e.deltaY < 0) {
        increaseVolume();
    } else {
        decreaseVolume();
    }
}, 125);

const showVideoChooser = () => {
    choosingVideo.value = true;

    if (playing.value) {
        playVideo();
    }
};

const continueFromPrevious = () => {
    const previousVideo = history.value.video;
    const previousTime = history.value.time;
    const previousTitle = history.value.title;

    if (typeof previousTime === "number") {
        currentTime.value = previousTime;
    } else {
        currentTime.value = 0;
    }

    if (previousVideo) {
        setVideoSource(previousVideo, previousTitle);
    } else {
        showVideoDialog();
    }
};

const handleVideoError = () => {
    add({
        text: "Video url invalid",
        type: "error",
        timeout: 10000
    });
};

onBeforeMount(async () => {
    let videoPath: string = "";

    // Handles the IPC event emitted when the user picks a youtube video
    listen("video-downloaded", async (event) => {
        history.value.isYoutube = true;

        const payload = event.payload as { path: string; code: string };

        const temp = await basename(payload.path);
        const index = temp.indexOf(`[${payload.code}]`);
        const title = temp.substring(0, index - 1);

        history.value.youtubeCode = payload.code;

        setVideoSource(payload.path, title);
    });

    await getMatches().then((matches) => {
        videoPath = matches.args["videoPath"].value as string;
    });

    if (videoPath && (await exists(videoPath))) {
        const extension: string = await extname(videoPath);

        // Sets the video if the user does Open With or calls the program with a CLI path, otherwise shows the file dialog
        if (VALID_EXTENSIONS.includes(await extension.toLowerCase().substring(1, extension.toLowerCase().length))) {
            setVideoSource(videoPath);
            return;
        }
    }

    showVideoChooser();
});

const history = useLocalStorage<History>("history", {
    volume: 0.5,
    time: 0,
    title: undefined,
    isYoutube: false,
    video: undefined,
    youtubeCode: undefined
});

watch(volume, (newValue) => {
    history.value.volume = newValue;
});

watch(currentTime, (newValue) => {
    history.value.time = newValue;
});

watch(videoTitle, (newValue) => {
    history.value.title = newValue;
});

watch(
    () => history.value.video,
    (newValue) => {
        if (!newValue && videoPlayer.value) {
            if (playing.value) {
                playVideo();
            }

            showVideoChooser();
        }
    }
);

onMounted(() => {
    window.addEventListener("keydown", keyDownEventHandler);
    window.addEventListener("keyup", keyUpEventHandler);
    window.addEventListener("mousemove", mouseMoveHandler);
    window.addEventListener("wheel", wheelHandler);

    if (typeof history.value.volume === "number") {
        volume.value = history.value.volume;
    }
});

onUnmounted(() => {
    window.removeEventListener("keydown", keyDownEventHandler);
    window.removeEventListener("keyup", keyUpEventHandler);
    window.removeEventListener("mousemove", mouseMoveHandler);
    window.addEventListener("wheel", wheelHandler);
});
</script>

<template>
    <NotificationRenderer />
    <VideoChooser
        v-if="choosingVideo"
        :previousVideo="history.video"
        @local="showVideoDialog"
        @youtube="showYoutubeModal"
        @previous="continueFromPrevious"
        @quit="getCurrentWebviewWindow().close()"
    />
    <!-- Top bar -->
    <div class="bg-charcoal min-h-[30px] flex gap-1" v-if="!isFullscreen">
        <button @click="showHelpWindow" class="aspect-square w-[30px] p-1">
            <div class="flex items-center justify-center">
                <Info color="white" fill="#1958b7" :strokeWidth="1.5" />
            </div>
        </button>
        <button ref="youtubeButton" @click="showYoutubeModal" class="aspect-square w-[30px] p-1">
            <div class="flex items-center justify-center">
                <Youtube fill="red" color="white" :strokeWidth="1.5" />
            </div>
        </button>
        <button ref="homeButton" @click="showVideoChooser" class="aspect-square w-[30px] p-1">
            <div class="flex items-center justify-center">
                <Home class="inline-block" color="white" :strokeWidth="1.5" />
            </div>
        </button>
        <button v-if="history.isYoutube" ref="saveButton" @click="saveYouTubeVideo" class="aspect-square w-[30px] p-1">
            <div class="flex items-center justify-center">
                <Save color="white" />
            </div>
        </button>
        <div class="w-full flex justify-center items-center text-white">
            <div class="flex flex-grow justify-center cursor-grab" data-tauri-drag-region>
                <span
                    v-if="videoTitle"
                    class="select-none cursor-pointer p-1 px-2 outline outline-1 rounded-md m-1 text-sm bg-black hover:bg-charcoal"
                    title="Change Video"
                    @click="showVideoChooser"
                >
                    {{ videoTitle }}
                </span>
            </div>
            <div class="w-[100px]"></div>
        </div>
        <div class="fixed top-0 right-0 flex items-center h-[36px] justify-around flex-grow-0 w-[100px]">
            <Minus
                class="cursor-pointer text-white hover:text-slate-300"
                @click="() => getCurrentWebviewWindow().minimize()"
                :size="20"
            />
            <Square
                class="cursor-pointer text-white hover:text-slate-300"
                @click="
                    async () =>
                        (await getCurrentWebviewWindow().isMaximized())
                            ? getCurrentWebviewWindow().unmaximize()
                            : getCurrentWebviewWindow().maximize()
                "
                :size="20"
            />
            <X
                class="cursor-pointer text-white hover:text-slate-300"
                @click="() => getCurrentWebviewWindow().close()"
                :size="20"
            />
        </div>
    </div>
    <!-- Playback rate -->
    <span v-if="!uiHidden" :class="`select-none absolute ${isFullscreen ? 'top-0' : 'top-[30px]'}`">{{
        rate.toFixed(2)
    }}</span>
    <!-- Transformation alert -->
    <Transition name="fade">
        <div
            v-if="transformationIcon"
            class="rounded-full bg-black outline outline-1 outline-white bg-opacity-50 flex flex-col justify-evenly items-center absolute m-4 right-0 aspect-square w-[100px]"
        >
            <div class="w-5"><component :is="transformationIcon" color="white" /></div>
            <span v-if="transformationText" class="text-white">{{ transformationText }}</span>
        </div>
    </Transition>
    <!-- Video player -->
    <div @click="playVideo" class="bg-black max-h-full h-full">
        <video ref="videoPlayer" class="max-h-full max-w-full w-full" @error="handleVideoError"></video>
    </div>
    <!-- Progress bar -->
    <div class="flex flex-col w-[95%] bottom-[140px] relative mx-auto" :style="{ display: uiHidden ? 'none' : 'flex' }">
        <div
            class="flex h-[14px] items-center z-40 cursor-pointer"
            @click="
                (e) => {
                    progressBar
                        ? seekVideoSection((e.x - progressBar.getBoundingClientRect().x) / progressBar.clientWidth)
                        : undefined;
                }
            "
        >
            <div
                ref="progressBar"
                class="bg-black w-full h-[5px] bg-opacity-50"
                style="background: rgba(255, 255, 255, 0.5)"
            >
                <div class="w-0 bg-red-600 h-[5px]" :style="{ width: `${(currentTime / duration) * 100}%` }"></div>
                <div
                    ref="progressCircle"
                    class="rounded-full aspect-square w-4 bg-red-600 outline outline-1 outline-white absolute top-0 cursor-grab z-50"
                    :style="{
                        left: `${circlePosition}%`
                    }"
                ></div>
            </div>
        </div>
        <!-- Video information -->
        <div class="mt-1 flex justify-between">
            <span class="pointer-events-none">{{ toHHMMSS(Math.round(currentTime).toString()) }}</span>
            <span>{{ toHHMMSS(Math.round(duration).toString()) }}</span>
        </div>
    </div>
    <!-- Video controls -->
    <div
        :class="`w-full h-[80px] flex fixed bottom-0 left-0 items-center justify-between pl-8 pr-8 bg-gradient-to-b from-transparent to-black ${
            uiHidden ? 'hidden' : ''
        }`"
    >
        <div class="flex justify-start flex-1">
            <button @click="setLoopMode" class="aspect-square w-[75px]">
                <div class="flex items-center justify-center">
                    <Repeat :color="looping ? 'limegreen' : 'white'" />
                </div>
            </button>
            <button @click="volume ? mute() : unmute()" class="aspect-square w-[75px]">
                <div class="flex items-center justify-center">
                    <VolumeX v-if="!volume" color="white" />
                    <Volume2 v-else color="white" />
                </div>
            </button>
        </div>
        <div class="flex justify-center flex-1">
            <button @click="() => rewindVideo(5)" class="aspect-square w-[75px]">
                <div class="flex items-center justify-center">
                    <Rewind color="white" />
                </div>
            </button>
            <button @click="playVideo" class="aspect-square w-[75px]">
                <div class="flex items-center justify-center">
                    <Play v-if="!playing" color="white" />
                    <Pause v-else color="white" />
                </div>
            </button>
            <button @click="() => forwardVideo(5)" class="aspect-square w-[75px]">
                <div class="flex items-center justify-center">
                    <FastForward color="white" />
                </div>
            </button>
        </div>
        <div class="flex justify-end flex-1">
            <button @click="() => setFullscreen()" class="aspect-square w-[75px]">
                <div class="flex items-center justify-center">
                    <Minimize color="white" v-if="isFullscreen" />
                    <Maximize color="white" v-else />
                </div>
            </button>
        </div>
    </div>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
    transition: opacity 0.5s ease;
}

.fade-enter-from,
.fade-leave-to {
    opacity: 0;
}
</style>
