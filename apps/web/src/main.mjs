import { loadStoryOSWebState } from "./app.mjs";
import { mountStage1View } from "./stage1-view.jsx";

const loaded = await loadStoryOSWebState();
mountStage1View(loaded.root, loaded);
