import { useEffect, useState } from "react";
import { api, type BackupStatus, type Finding } from "./api";

/**
 * GitHub 백업 확인창.
 *
 * 토큰을 묻지 않는다. 이미 로그인된 `gh`를 빌려 쓴다.
 * 공개 여부는 기본이 비공개이고, 공개는 사용자가 직접 골라야 한다 —
 * 실수로 세상에 공개하는 쪽이 훨씬 비싸다.
 */
export function Backup({
  onClose,
  onDone,
}: {
  onClose: () => void;
  onDone: (message: string) => void;
}) {
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [blocking, setBlocking] = useState<Finding[] | null>(null);
  const [makePublic, setMakePublic] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.backupStatus().then(setStatus).catch((e) => setError(String(e)));
    api.backupGuard().then(setBlocking).catch(() => setBlocking([]));
  }, []);

  async function run() {
    setBusy(true);
    setError(null);
    try {
      const done = await api.backupRun(!makePublic);
      onDone(
        done.created
          ? `GitHub에 새로 만들고 ${done.backed_up}개를 올렸어요.`
          : `${done.backed_up}개를 올렸어요.`,
      );
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const ready = status?.readiness.state === "ready";
  const blocked = (blocking?.length ?? 0) > 0;

  return (
    <div className="scrim" onClick={busy ? undefined : onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <span className="chip accent">GitHub 백업</span>

        {!status && !error && <p>확인하는 중…</p>}

        {status?.readiness.state === "noTool" && (
          <>
            <h3>GitHub에 올리려면 준비가 하나 필요해요</h3>
            <p>
              터미널에서 <code>brew install gh</code> 를 실행하고,{" "}
              <code>gh auth login</code> 으로 한 번 로그인해 주세요. 그 뒤로는
              Kigtit이 알아서 씁니다.
            </p>
            <div className="reassure">
              <span>🔑</span>
              <span>
                <b>토큰을 만들거나 붙여넣을 필요는 없어요.</b> Kigtit은 이미 로그인된
                것을 빌려 쓰기만 합니다.
              </span>
            </div>
          </>
        )}

        {status?.readiness.state === "notSignedIn" && (
          <>
            <h3>GitHub 로그인이 한 번 필요해요</h3>
            <p>
              터미널에서 <code>gh auth login</code> 을 실행해 주세요. 끝나면 이
              창을 다시 열면 됩니다.
            </p>
          </>
        )}

        {ready && (
          <>
            <h3>
              {status!.remote
                ? `세이브 포인트 ${status!.unbacked}개를 올릴까요?`
                : "GitHub에 백업을 시작할까요?"}
            </h3>
            <p>
              {status!.readiness.state === "ready" && (
                <>
                  <b>{status!.readiness.account}</b> 계정으로 올립니다.{" "}
                </>
              )}
              {status!.remote
                ? "이미 연결된 곳에 이어서 올려요."
                : "이 폴더 이름으로 저장소를 새로 만들어요."}
            </p>

            {blocked && (
              <>
                <div className="reassure warn">
                  <span>🔐</span>
                  <span>
                    <b>비밀 키가 들어 있어서 올릴 수 없어요.</b> 한 번 올라간 키는
                    몇 분 안에 남이 긁어 갑니다.
                  </span>
                </div>
                <div className="findings">
                  {blocking!.map((f) => (
                    <div className="finding" key={f.path + f.line}>
                      <span>{f.message}</span>
                      {f.masked && <code>{f.masked}</code>}
                    </div>
                  ))}
                </div>
              </>
            )}

            {!blocked && !status!.remote && (
              <>
                <label className="choice">
                  <input
                    type="checkbox"
                    checked={makePublic}
                    onChange={(e) => setMakePublic(e.target.checked)}
                  />
                  <span>누구나 볼 수 있게 만들기</span>
                </label>
                <div className={makePublic ? "reassure warn" : "reassure"}>
                  <span>{makePublic ? "🌏" : "🔒"}</span>
                  <span>
                    {makePublic ? (
                      <>
                        <b>세상에 공개됩니다.</b> 코드와 지금까지의 모든 세이브
                        포인트를 누구나 볼 수 있어요.
                      </>
                    ) : (
                      <>
                        <b>나만 볼 수 있어요.</b> 나중에 GitHub에서 공개로 바꿀 수
                        있습니다.
                      </>
                    )}
                  </span>
                </div>
              </>
            )}
          </>
        )}

        {error && <p style={{ color: "var(--bad)" }}>{error}</p>}

        <div className="modal-actions">
          <button className="btn ghost" onClick={onClose} disabled={busy}>
            {ready && !blocked ? "그만두기" : "닫기"}
          </button>
          {ready && !blocked && (
            <button className="btn primary" onClick={run} disabled={busy}>
              {busy ? "올리는 중…" : makePublic ? "공개로 올리기" : "올리기"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
