import { createApp } from "vue";
import "./styles.css";
import App from "./App.vue";

import translations from "./translations";
import i18next from "i18next";
import I18NextVue from "i18next-vue";

import { router } from "./router";
import { useNotification } from "./composables";

i18next.init(translations);

const { add } = useNotification();

const app = createApp(App);

app.config.errorHandler = (err: unknown) => {
    console.log(err);
    add({
        text: (err as Error).message,
        type: "error",
        timeout: -1
    });
};

app.use(router).use(I18NextVue, { i18next }).mount("#app");
