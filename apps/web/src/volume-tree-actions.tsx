import { updateOwnedVolume } from "./update-volume.ts";

export function VolumeTreeActions({
  projectId,
  volumeId,
  title,
  order,
  volumeCount,
  expectedVolumeRevision,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onUpdated,
}: {
  projectId: string;
  volumeId: string;
  title: string;
  order: string;
  volumeCount: number;
  expectedVolumeRevision: string;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onUpdated: () => void;
}) {
  const currentOrder = Number(order);
  const canMove = Number.isInteger(currentOrder) && currentOrder >= 1;
  const submitUpdate = (nextTitle: string, nextOrder: string) => {
    void updateOwnedVolume({
      baseUrl,
      fetchImpl,
      cryptoImpl,
      projectId,
      volumeId,
      title: nextTitle,
      order: nextOrder,
      expectedVolumeRevision,
    }).then((updated) => {
      if (
        updated.effect.kind !== "authoritative_applied"
        && updated.effect.kind !== "no_effect"
      ) {
        return;
      }
      onUpdated();
    }).catch(() => {});
  };
  return (
    <>
      <form
        data-rename-volume={volumeId}
        onSubmit={(event) => {
          event.preventDefault();
          const nextTitle = String(new FormData(event.currentTarget).get("volume-title") ?? "").trim();
          if (!nextTitle) return;
          submitUpdate(nextTitle, order);
        }}
      >
        <label>
          卷标题
          <input name="volume-title" required maxLength={1024} defaultValue={title} />
        </label>
        <button type="submit">重命名</button>
      </form>
      <button
        type="button"
        data-volume-move="up"
        disabled={!canMove || currentOrder <= 1}
        onClick={() => {
          if (!canMove || currentOrder <= 1) return;
          submitUpdate(title, String(currentOrder - 1));
        }}
      >
        上移
      </button>
      <button
        type="button"
        data-volume-move="down"
        disabled={!canMove || currentOrder >= volumeCount}
        onClick={() => {
          if (!canMove || currentOrder >= volumeCount) return;
          submitUpdate(title, String(currentOrder + 1));
        }}
      >
        下移
      </button>
    </>
  );
}
