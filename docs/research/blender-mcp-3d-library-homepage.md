# Blender MCP 与 StoryOS 3D 图书馆首页调研

状态：2026-07-15 调研结论，未实施。

## 结论

**可以做，而且概念与 StoryOS 的“一个项目就是一本小说”非常契合；但 Blender MCP 只能作为设计期的 3D 制作工具，不能充当网页运行时或项目数据层。**

推荐的产品形态是：

- StoryOS 登录后的首页是一个宿主自有的项目选择页面；
- 浏览器中的实时 3D 图书馆负责空间感、书架概览、选书和聚焦动画；
- 普通 HTML/React 界面负责搜索、新建项目、最近编辑信息、键盘操作、无障碍语义和低性能降级；
- Blender 负责制作房间、书架、灯光和少量静态装饰，导出 `.glb`；
- StoryOS 项目数据在运行时生成每本书及其标题、状态和位置；
- Blender MCP 只帮助 Codex 探索、修改和检查 Blender 场景，不进入 StoryOS 生产运行时。

因此，准确的组合不是“用 Blender MCP 做一个网页”，而是：

> Codex 编写和验证网页交互；Codex 通过 Blender MCP 辅助制作 3D 美术资产；浏览器通过 Three.js/React Three Fiber 加载资产；StoryOS Core/API 提供项目数据。

## 当前 StoryOS 中的位置

仓库当前明确的是**项目内部**三栏写作工作区：左侧稿件树、中间编辑器、右侧 Agent 对话；现有可运行原型也是 React 19 + Vite 6 的项目内验证原型。参见 [三栏工作区设计](../design/storyos-three-column-writing-workspace.md) 与 [原型说明](../../prototypes/tiptap-proposal-lab/NOTES.md)。

新的 3D 图书馆则是**项目外、登录后的宿主首页**，职责只有项目发现、选择、创建和进入。它不应成为 MCP App，也不拥有小说或项目元数据的权威状态；3D 书本只是 StoryOS 项目记录的可视化投影。

这一边界符合仓库现有产品不变量：本地项目数据是权威来源，外部工具只获得当前步骤所需的最小上下文。

## Codex 与 Blender MCP 能做什么

### Codex 侧：技术上可以连接

Codex 当前官方文档说明，本地 Codex 客户端支持本地进程形式的 STDIO MCP server，也支持 Streamable HTTP MCP server；ChatGPT 桌面端、Codex CLI 和 IDE 扩展可共享同一个 Codex host 的 MCP 配置。因此，只要 BlenderMCP 提供标准 STDIO server，Codex 具备连接它的基础能力。[Codex MCP 官方文档](https://learn.chatgpt.com/docs/extend/mcp.md)

本机在调研时尚未安装 Blender，`codex mcp list` 中也没有 Blender MCP；这里确认的是可行性，不是已经完成的本机集成。

### BlenderMCP 侧：它是社区项目，不是 Blender 官方组件

本调研所称 BlenderMCP 是 [`ahujasid/blender-mcp`](https://github.com/ahujasid/blender-mcp)。上游明确将自己描述为第三方集成，而非 Blender 官方产品；其结构是 Blender 插件内的 socket server，加上对 MCP 客户端暴露工具的 Python server。[上游 README：组件与免责声明](https://github.com/ahujasid/blender-mcp#components)

它当前可读取场景/对象信息、获取视口截图、创建或修改对象与材质，并通过 `execute_blender_code` 在 Blender 中运行 Python；也可选择接入 Poly Haven、Sketchfab 与外部 3D 生成服务。[上游工具源码](https://github.com/ahujasid/blender-mcp/blob/6641189231caf3752302ae20591bc87fda85fc4e/src/blender_mcp/server.py#L255-L393) [上游 README：能力](https://github.com/ahujasid/blender-mcp#capabilities)

这足以让 Codex 辅助完成：

- 生成低多边形房间、书架和占位书；
- 调整材质、灯光、相机和构图；
- 通过截图检查当前视口并迭代；
- 用 Blender Python 调用导出流程。

它没有把 Blender 变成网页引擎，也没有替代 Three.js 的交互、路由、状态管理、无障碍和性能工程。当前上游也没有独立的 glTF 导出 MCP 工具；导出通常需要通过任意 Blender Python 工具或人工 Blender 操作完成。[当前 MCP 工具定义](https://github.com/ahujasid/blender-mcp/blob/6641189231caf3752302ae20591bc87fda85fc4e/src/blender_mcp/server.py)

## 推荐交付架构

```text
StoryOS 项目 API / 本地 Core
  -> ProjectSummary[]
  -> React 首页状态（selectedProjectId、搜索、排序、路由）
     -> HTML/DOM 层：搜索、新建、详情、无障碍项目列表、降级入口
     -> React Three Fiber / Three.js Canvas
        -> 加载 Blender 导出的 library-shell.glb
        -> 按 ProjectSummary 动态生成书本实例
        -> 拾取书本、聚焦相机、悬停/焦点反馈
  -> /projects/:projectId
  -> 现有三栏写作工作区
```

### 静态资产与动态数据必须分开

Blender 资产只包含通用环境：房间、书架、台阶、灯具、少量装饰、命名锚点和可选相机参考位。项目书本不应全部在 `.blend` 中手工制作，否则新增、删除、排序项目都会变成美术资产修改。

更稳妥的方案是：

- `.glb` 内保存书架槽位或命名锚点；
- Web 运行时根据 `ProjectSummary[]` 在槽位生成书本；
- 书本共用少量 geometry/material，项目 ID 只决定映射和交互；
- 标题、更新时间、进度等使用可访问的 DOM 详情层呈现，不依赖用户读清 3D 贴图文字；
- 书本位置由显式排序规则或持久化 shelf slot 决定，而不是 Blender 文件成为项目目录。

Blender 官方 glTF 导出器支持 mesh、PBR 材质、纹理、相机、点/聚光/方向光与动画，并可导出单文件 `.glb`；glTF 本身是面向 Web 与原生运行时传输的格式。[Blender glTF 手册](https://docs.blender.org/manual/en/3.3/addons/import_export/scene_gltf2.html) [Khronos glTF 说明](https://www.khronos.org/gltf/)

Three.js 官方 `GLTFLoader` 可加载 glTF 2.0，`Raycaster` 用于从鼠标/指针位置拾取 3D 对象；`InstancedMesh` 适合大量共享 geometry/material 的书本，从而减少 draw call。[GLTFLoader](https://threejs.org/docs/pages/GLTFLoader.html) [Raycaster](https://threejs.org/docs/pages/Raycaster.html) [InstancedMesh](https://threejs.org/docs/pages/InstancedMesh.html)

现有原型已经使用 React 19；React Three Fiber 是 Three.js 的 React renderer，其 v9 与 React 19 配套，因此是最自然的原型技术选择，但不是必须的生产承诺。[React Three Fiber 官方介绍](https://r3f.docs.pmnd.rs/getting-started/introduction)

## 建议的交互

### 默认状态

- 相机显示一个安静、可读的书架区域，而不是让用户自由第一人称漫游。
- 页面始终保留“继续最近项目”“搜索”“新建项目”和账户入口。
- 最近项目应该最快到达；沉浸感不能迫使用户每次穿过整间图书馆。

### 选择一本书

1. 指针点击或键盘选择书本；
2. `selectedProjectId` 成为唯一选择状态；
3. 相机平滑移动到预先计算的 framing pose，让书本成为焦点；
4. DOM 详情层显示标题、最近编辑、章节/进度摘要和“继续写作”；
5. 再次确认或按 Enter 进入 `/projects/:projectId`；
6. Escape/返回回到书架概览。

不要把“点击一次就立刻进入项目”与“点击后聚焦欣赏”混为同一动作，否则用户无法理解相机运动的意义。也不要用自由 OrbitControls 作为默认项目管理导航；那更像 3D 查看器，不像稳定的产品首页。

### 搜索与大量项目

3D 体验对少量项目很有魅力，但几十到几百个项目会暴露定位和记忆负担。搜索结果应能直接选择项目并让相机跳转到对应书本；同时保留普通列表视图或可访问项目列表。3D 可以是默认表现，但不能是唯一索引。

## 视觉方向

现有工作区的长期视觉基线是安静、文学、暖白、低 chrome，而不是游戏 UI 或技术仪表盘。3D 首页可以沿用这一性格：克制的建筑空间、低饱和材质、柔和光照、少量有意义的动画。应先做灰盒验证，不应先投入写实木纹、粒子、景深和复杂后期效果。

一个重要判断是：**图书馆隐喻本身已经足够强，不需要再堆“魔法书、漂浮粒子、NPC、任务标记”等游戏化符号来证明它是 3D。**

## 无障碍与性能边界

- 每个项目必须同时存在可聚焦的 DOM 入口；Canvas 里的像素和 mesh 不能成为唯一语义。
- 键盘、屏幕阅读器和触屏用户必须能完成搜索、选择、查看详情和进入项目。
- 尊重 `prefers-reduced-motion`：相机移动改为立即 framing 或短淡入淡出，并允许完全关闭环境动画。[MDN `prefers-reduced-motion`](https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-motion)
- 低性能或 WebGL 初始化失败时，直接显示普通项目列表，而不是阻止用户进入作品。
- 书本尽量共享 geometry/material；控制纹理尺寸、灯光数量、阴影和后期处理。
- 静态首页不应永久 60 FPS 空转；Three.js 官方建议在非持续动画场景中按需渲染以节省算力和电量。[Three.js 按需渲染](https://threejs.org/manual/en/rendering-on-demand.html)
- 切换或卸载场景时必须显式释放 geometry、material 和 texture；Three.js 不会自动释放所有 GPU 资源。[Three.js 资源清理](https://threejs.org/manual/en/how-to-dispose-of-objects.html)

## BlenderMCP 安全边界

BlenderMCP 只应在隔离的设计环境里使用，并视为具有本机代码执行能力的开发工具：

- 上游明确说明 `execute_blender_code` 能运行任意 Python，并提醒生产环境风险。[上游安全说明](https://github.com/ahujasid/blender-mcp#limitations--security-considerations)
- 上游遥测在同意状态下可包含 prompts、代码片段和截图；应设置 `DISABLE_TELEMETRY=true`，而不是把小说内容、私有路径或凭据发给它。[上游遥测说明](https://github.com/ahujasid/blender-mcp#telemetry-control)
- NVD 的 [CVE-2026-10662](https://nvd.nist.gov/vuln/detail/CVE-2026-10662) 记录了 Hunyuan3D ZIP URL 处理的 SSRF；截至本调研，修复 PR [#205](https://github.com/ahujasid/blender-mcp/pull/205) 仍未合并。
- 调研当日上游 `main` 为 `6641189231caf3752302ae20591bc87fda85fc4e`，对应代码仍只检查 `http(s)` scheme 后调用 `requests.get(zip_file_url)`。[当前代码](https://github.com/ahujasid/blender-mcp/blob/6641189231caf3752302ae20591bc87fda85fc4e/addon.py#L2342-L2360)

如果进行实验：

1. 固定并审阅具体 commit，不使用无法复现的浮动 `uvx latest` 作为信任边界；
2. 只绑定 localhost，不开放 Blender socket 到局域网/公网；
3. 在 Codex MCP 配置中使用 `enabled_tools` 限制工具；不需要的外部资产与 Hunyuan/Hyper3D/Sketchfab 工具全部禁用；
4. 使用副本 `.blend`、独立输出目录和无敏感数据的提示；
5. 完全关闭遥测；
6. 复杂场景生成完成后，审阅并保存 `.blend`、导出脚本和导出产物，不把 MCP 对话当作资产来源记录。

StoryOS 的生产应用不应依赖运行中的 Blender、BlenderMCP socket 或外部 3D 生成服务。

## 最小验证顺序

### POC 1：先回答产品问题，不安装 Blender

在一个新的 disposable Web 原型中使用 React Three Fiber 和简单 box geometry：

- 固定一段书架与 8–12 本占位书；
- overview、focused、opening 三个明确相机/选择状态；
- 点击书本后聚焦，Escape 返回，Enter 进入占位项目路由；
- DOM 搜索、项目详情、“继续写作”和“新建项目”；
- 键盘选择、`prefers-reduced-motion` 和普通列表降级；
- 用假 `ProjectSummary[]` 验证新增/删除/排序不会修改场景资产。

这一阶段的验收问题只有三个：

1. 聚焦一本书是否真的比普通卡片更有情感价值？
2. 用户是否仍能比普通项目列表更快或同样快地继续最近作品？
3. 相机运动在反复使用后是否仍然舒服，而不是等待动画？

若这三个问题没有得到肯定答案，应停止，不进入 Blender 美术制作。

### POC 2：通过后再引入 Blender MCP

- 安装隔离的 Blender 与审阅/固定版本的 BlenderMCP；
- 用 Codex + BlenderMCP 制作一段低多边形、暖中性书架空间；
- 保存源 `.blend`，用可重复的 Blender Python/固定导出设置生成 `.glb`；
- Web 原型替换静态环境，动态书本与交互状态保持不变；
- 检查桌面与移动端加载、GPU/内存、首次交互时间和降级路径。

## 最终建议

**建议继续，但下一步不是安装 Blender MCP，而是先做 Web 3D 灰盒。**

这个灰盒能以最低代价验证真正高风险的部分：3D 首页是否比平面项目页更好用、聚焦动画是否耐用、3D 与普通管理控件能否共存。只有交互成立后，Blender MCP 才有明确任务：把已经成立的空间交互换成高质量、可导出、可复现的 StoryOS 图书馆环境。
