"""Generate original Chinese prose with an exact scalar budget and a fixed seed."""
import hashlib
import json
from pathlib import Path
import random
import statistics
import sys

SEED = 20261005
SCALES = (30000, 300000, 1000000, 3000000)
out = Path(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).parent / 'out')
out.mkdir(parents=True, exist_ok=True)
rng = random.Random(SEED)
names = ['沈青', '林远', '陆先生', '阿宁', '小舟', '老掌柜', '守夜人', '旅客']
places = ['石桥边', '山门外', '长街尽头', '雨中的庭院', '渡口', '旧书房', '城南', '河岸']
verbs = ['看见', '想起', '找到了', '终于听见', '仍然记得', '悄悄收起', '回头望着']
objects = ['那封没有署名的信', '远处的灯火', '窗前晃动的影子', '一枚旧铜钱', '昨日留下的脚印', '深夜传来的钟声']
tails = ['风从门缝里吹了进来。', '谁也没有再说话。', '这件事还远远没有结束。', '天色渐渐暗了下来。', '他知道，自己必须作出选择。']

def sentence():
    if rng.random() < .28:
        return f'“{rng.choice(["你还记得吗", "先等一等", "我们该走了", "这里究竟发生了什么"])}？”{rng.choice(names)}低声问道。'
    return f'{rng.choice(names)}在{rng.choice(places)}{rng.choice(verbs)}{rng.choice(objects)}，{rng.choice(tails)}'

chapters = []
for group in range(300):
    lengths = [rng.randint(1600, 2400) for _ in range(4)]
    lengths.append(10000 - sum(lengths))
    for length in lengths:
        paragraphs, remaining = [], length
        while remaining:
            goal = min(remaining, rng.choice([28, 48, 72, 96, 128, 160]))
            text = '　　' if rng.random() < .12 else ''
            while len(text) < goal:
                text += sentence()
            text = text[:goal - 1] + '。'
            if remaining > goal:
                if remaining - goal == 1:
                    text += '。'
                else:
                    remaining -= 1
            paragraphs.append(text)
            remaining -= len(text)
        index = len(chapters) + 1
        if index % 10 == 0:
            p = paragraphs[0]
            paragraphs[0] = '第2026号ABC线索：' + p[len('第2026号ABC线索：'):]
        chapters.append({'title': f'第{index:04d}章 夜雨与归途', 'paragraphs': paragraphs})

manifest = []
for scale in SCALES:
    selected = chapters[:scale // 2000]
    bodies = ['\n'.join(c['paragraphs']) for c in selected]
    data = {'seed': SEED, 'scale': scale, 'size_unit': 'Unicode scalar values in chapter bodies, including LF separators', 'chapters': selected}
    raw = json.dumps(data, ensure_ascii=False, separators=(',', ':')).encode()
    (out / f'corpus-{scale}.json').write_bytes(raw)
    sizes = [len(p) for c in selected for p in c['paragraphs']]
    manifest.append({'scale': scale, 'chapters': len(selected), 'scalars': sum(map(len, bodies)), 'utf8_bytes': sum(len(t.encode()) for t in bodies), 'blocks': len(sizes), 'paragraph_min': min(sizes), 'paragraph_median': statistics.median(sizes), 'paragraph_max': max(sizes), 'chapter_min': min(map(len, bodies)), 'chapter_max': max(map(len, bodies)), 'sha256': hashlib.sha256(raw).hexdigest()})
(out / 'corpus-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps(manifest, indent=2))
