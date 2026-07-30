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
          ? `Created a GitHub repository and uploaded ${done.backed_up} save points.`
          : `Uploaded ${done.backed_up} save points.`,
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
        <span className="chip accent">GitHub backup</span>

        {!status && !error && <p>Checking…</p>}

        {status?.readiness.state === "noTool" && (
          <>
            <h3>One thing is needed before uploading to GitHub</h3>
            <p>
              Run <code>brew install gh</code> in a terminal, then sign in once with{" "}
              <code>gh auth login</code>. Kigtit will use that login from then on.
            </p>
            <div className="reassure">
              <span>🔑</span>
              <span>
                <b>You don't need to create or paste a token.</b> Kigtit only uses your
                existing login.
              </span>
            </div>
          </>
        )}

        {status?.readiness.state === "notSignedIn" && (
          <>
            <h3>Sign in to GitHub once</h3>
            <p>
              Run <code>gh auth login</code> in a terminal, then reopen this window.
            </p>
          </>
        )}

        {ready && (
          <>
            <h3>
              {status!.remote
                ? `Upload ${status!.unbacked} save points?`
                : "Start backing up to GitHub?"}
            </h3>
            <p>
              {status!.readiness.state === "ready" && (
                <>
                  Uploading with the <b>{status!.readiness.account}</b> account.{" "}
                </>
              )}
              {status!.remote
                ? "New save points will go to the connected repository."
                : "A new repository will be created with this folder's name."}
            </p>

            {blocked && (
              <>
                <div className="reassure warn">
                  <span>🔐</span>
                  <span>
                    <b>Can't upload because a secret key was found.</b> Exposed keys can be
                    scraped within minutes.
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
                  <span>Make this visible to everyone</span>
                </label>
                <div className={makePublic ? "reassure warn" : "reassure"}>
                  <span>{makePublic ? "🌏" : "🔒"}</span>
                  <span>
                    {makePublic ? (
                      <>
                        <b>This will be public.</b> Anyone can see the code and every save point.
                      </>
                    ) : (
                      <>
                        <b>Only you can see it.</b> You can make it public later on GitHub.
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
            {ready && !blocked ? "Cancel" : "Close"}
          </button>
          {ready && !blocked && (
            <button className="btn primary" onClick={run} disabled={busy}>
              {busy ? "Uploading…" : makePublic ? "Upload publicly" : "Upload"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
