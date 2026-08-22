import { loadStoryOSWebState } from "./app.ts";
import { mountStage1View } from "./stage1-view.tsx";

const loaded = await loadStoryOSWebState();
mountStage1View(loaded.root, loaded);
