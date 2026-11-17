"""Makes the constant sample manuscript of the second live run of #1600.

The filler comes from a sentence bank with a fixed seed. One target paragraph is written by hand.
The text is not a work of the author and not a copy of a published text.
"""
import hashlib, json, random, sys

SEED = 1600
TITLES = ["春网", "修船", "盐价", "祠堂", "晒场", "换工", "学堂", "雨季", "账本", "新桥",
          "腌菜", "旧债", "米铺", "嫁妆", "木匠", "鱼市", "冬至", "分家", "过年", "开春"]
TARGET_CHAPTER = 11   # 旧债, the twelfth Chapter
TARGET_PARAGRAPH = 3  # the fourth paragraph, from 0

TARGET = (
    "那年秋汛最凶的一晚，河面上白茫茫一片，站在码头上连对岸的柳树都望不到。出港的七条渔船，到了该进港的时辰，一条也不曾靠岸。"
    "男人们都在船上，镇上只剩些老人和孩子，大伙挤在祠堂门口，谁也说不出话，只听得见河水一下一下拍着石阶。"
    "秦婆婆不去祠堂，她从柴房里抱出一捆浸过松脂的芦苇，扛到老码头尽头的石墩上点着，又一捆接一捆地往上添。"
    "孙木匠的小儿子跑来帮她搬柴，她叫孩子把柴房里剩下的芦苇都抱过来，一根也别留，自己守在石墩旁边，寸步不离，整整守到后半宿。"
    "那一团跳动的红在水汽里时隐时现，像有人在岸边不停地招手。风一阵紧过一阵，好几回眼看就要被吹灭，她就用身子挡着风，再添上一捆。"
    "过了很久，远处终于传来了橹声，先是一条，接着是两条、三条，船上的人顺着那团红，一条一条摸进了港湾。"
    "最后一条船靠岸时，天边已经发白，秦婆婆的眉毛被烤焦了一半，手背上也起了泡，她却只问了一句：都齐了吗？"
)

PEOPLE = ["林舟", "林雪", "老周", "阿满", "何三叔", "小桃", "钱掌柜", "孙木匠", "刘先生", "杜大娘"]
PLACES = ["集市", "晒网场", "渡口", "祠堂前的空地", "盐仓", "镇东的石桥", "米铺", "学堂", "河湾", "后山的菜地"]
THINGS = ["一筐鲜鱼", "两匹蓝布", "一袋新米", "半罐猪油", "一卷麻绳", "三斤粗盐", "一把新斧头", "几张草纸", "一篮鸡蛋", "一包茶叶"]
SENTENCES = [
    "{p}一早就去了{l}，想赶在人多之前把事情办完。",
    "{l}今天格外热闹，卖菜的和买菜的挤成一团，说话声一阵高过一阵。",
    "{p}在{l}碰见了{q}，两人站着聊了好一会儿家常。",
    "{p}算了算手里的钱，只够买{t}，别的只好等下个月再说。",
    "{q}说今年的收成比去年强一些，至少不用再向钱掌柜赊账了。",
    "{p}把{t}背回家，放在灶台边上，又去井边打了两桶水。",
    "太阳升到半空，{l}的石板被晒得发烫，孩子们赤着脚跑来跑去。",
    "{p}想起{q}前几天托他办的事，赶紧从怀里摸出那张写着字的纸条。",
    "{q}的院子里种着几棵枣树，秋天一到，枣子落了满地。",
    "镇上的人都知道{p}脾气好，谁家有活忙不过来，总会先去喊他。",
    "{p}和{q}为了{t}的价钱争了几句，最后各让一步，算是谈妥了。",
    "{l}边上新开了一家铺子，门口挂着黄纸写的招牌，字写得歪歪扭扭。",
    "{p}一边走一边盘算，明天要先去{l}，再去找{q}商量修房子的事。",
    "{q}的孩子刚满五岁，整天跟在大人身后，问这问那。",
    "到了晌午，{p}在{l}的树荫下吃了两个饼子，喝了一碗凉茶。",
    "{p}说话慢，可是办事从来不出错，钱掌柜最信得过的就是他。",
    "{q}把账本翻来覆去看了三遍，还是对不上那几文钱。",
    "{l}的老槐树下常有几个老人下棋，一盘棋能下一个下午。",
    "{p}托人从县城捎来{t}，说是给{q}家办喜事用的。",
    "镇上今年添了三户新人家，{l}一下子显得窄了不少。",
    "{p}蹲在门槛上剥豆子，{q}在一旁补衣服，两人有一句没一句地说着话。",
    "{q}说起年轻时去县城做工的事，说那时候一个月只能吃上两回肉。",
    "{p}把旧船板拆下来，挑出还能用的几块，堆在墙角晾着。",
    "{l}的路被雨水冲出了几道沟，{p}和{q}扛着锄头去填平。",
    "刘先生在学堂里教孩子们认字，念书声隔着两条街都听得见。",
    "{p}想给{q}送点{t}，又怕人家不收，在门口转了好几圈。",
    "钱掌柜的米铺新进了一批米，价钱比上个月便宜了一些。",
    "{q}家的鸡跑到了{p}家的菜地里，啄坏了一畦青菜。",
    "{p}笑着摆摆手，说不碍事，菜还能再长，邻里之间别伤了和气。",
    "孙木匠的手艺在镇上数一数二，谁家打柜子都要排队等他。",
]

def paragraph(rng, low, high):
    while True:
        text = ""
        while len(text) < low:
            p, q = rng.sample(PEOPLE, 2)
            s = rng.choice(SENTENCES).format(p=p, q=q, l=rng.choice(PLACES), t=rng.choice(THINGS))
            text += s
        if len(text) <= high:
            return text

def chapters():
    rng = random.Random(SEED)
    result = []
    for number, title in enumerate(TITLES):
        count = rng.choice([5, 6, 7])
        if number == TARGET_CHAPTER:
            count = 6
        low = {5: 560, 6: 470, 7: 410}[count]
        paragraphs = [paragraph(rng, low, low + 90) for _ in range(count)]
        if number == TARGET_CHAPTER:
            paragraphs[TARGET_PARAGRAPH] = TARGET
        result.append({"title": title, "paragraphs": paragraphs})
    return result

def cut(paragraphs, limit=800):
    passages, open_, length = [], [], 0
    for block in paragraphs:
        size = len(block)
        joined = size if not open_ else length + 1 + size
        if open_ and joined > limit:
            passages.append("\n".join(open_)); open_, length = [block], size
        else:
            open_.append(block); length = joined
    if open_:
        passages.append("\n".join(open_))
    return passages

if __name__ == "__main__":
    out = chapters()
    if sys.argv[1:] == ["json"]:
        print(json.dumps(out, ensure_ascii=False, indent=1))
    else:
        total = 0
        for i, ch in enumerate(out, 1):
            text = "\n".join(ch["paragraphs"])
            n = len(cut(ch["paragraphs"]))
            total += n
            print(i, ch["title"], len(ch["paragraphs"]), len(text), n,
                  "sha256:" + hashlib.sha256(text.encode()).hexdigest()[:12])
        print("passages", total, "characters", sum(len("\n".join(c["paragraphs"])) for c in out))
