import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";
import "../../shared-ui/ripple.css";
import { api } from "@botting/user-api";
import { watchUserCanvas } from "./lib/adapters/window";

const isMatchmakingOverlay = new URLSearchParams(window.location.search).get("overlay") === "matchmaking";
document.documentElement.dataset.userWindow = isMatchmakingOverlay ? "overlay" : "entry";
if (!isMatchmakingOverlay) watchUserCanvas();

void api.loadSettings().catch(() => ({})).then((settings) => {
  mount(App, { target: document.getElementById("app")!, props: { settings } });
});
