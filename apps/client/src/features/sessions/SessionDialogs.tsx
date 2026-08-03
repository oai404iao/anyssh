import { FormEvent, useState } from "react";
import type {
  AuthenticationChallengeEvent,
  HostKeyChangedEvent,
  HostKeyEvent,
} from "../../lib/ssh-bridge";
import {
  FingerprintIcon,
  ShieldIcon,
  WarningIcon,
} from "../../shared/icons/ProductIcons";

interface HostKeyDialogProps {
  event: HostKeyEvent;
  onDecision(accepted: boolean): Promise<void>;
}

export function HostKeyDialog({ event, onDecision }: HostKeyDialogProps) {
  return (
    <div className="dialog-backdrop">
      <section
        aria-labelledby="host-key-title"
        aria-modal="true"
        className="host-key-dialog"
        role="dialog"
      >
        <div className="dialog-icon">
          <FingerprintIcon />
        </div>
        <p className="eyebrow dialog-context-label">
          First connection ·{" "}
          {event.hop.kind === "target"
            ? "Target host"
            : `Jump host ${event.hop.index}`}
        </p>
        <h2 id="host-key-title">Verify server identity</h2>
        <p>
          This address has not been trusted on this device. Compare the
          fingerprint with a value from your server administrator.
        </p>
        <dl>
          <div>
            <dt>Host</dt>
            <dd>
              {event.host}:{event.port}
            </dd>
          </div>
          <div>
            <dt>Algorithm</dt>
            <dd>{event.algorithm}</dd>
          </div>
        </dl>
        <div className="fingerprint-card">
          <span>SHA-256 fingerprint</span>
          <code>{event.fingerprintSha256}</code>
        </div>
        <div className="dialog-security-note">
          <ShieldIcon />
          <span>
            If accepted, AnySSH remembers this identity and blocks unexpected
            changes on future connections.
          </span>
        </div>
        <div className="dialog-actions">
          <button
            className="secondary-button"
            onClick={() => void onDecision(false)}
            type="button"
          >
            Reject
          </button>
          <button
            className="connect-button"
            onClick={() => void onDecision(true)}
            type="button"
          >
            Trust and continue
          </button>
        </div>
      </section>
    </div>
  );
}

interface ChangedHostKeyDialogProps {
  event: HostKeyChangedEvent;
  onClose(): void;
  onOpenKnownHosts(): void;
}

export function ChangedHostKeyDialog({
  event,
  onClose,
  onOpenKnownHosts,
}: ChangedHostKeyDialogProps) {
  return (
    <div className="dialog-backdrop">
      <section
        aria-labelledby="changed-host-key-title"
        aria-modal="true"
        className="host-key-dialog changed-host-key-dialog"
        role="alertdialog"
      >
        <div className="dialog-icon danger-icon">
          <WarningIcon />
        </div>
        <p className="eyebrow dialog-context-label">
          Connection blocked ·{" "}
          {event.hop.kind === "target"
            ? "Target host"
            : `Jump host ${event.hop.index}`}
        </p>
        <h2 id="changed-host-key-title">Host key changed</h2>
        <p>
          The server presented a different identity. AnySSH stopped before
          authentication to protect the session.
        </p>
        <dl>
          <div>
            <dt>Host</dt>
            <dd>
              {event.host}:{event.port}
            </dd>
          </div>
          <div>
            <dt>Algorithm</dt>
            <dd>{event.algorithm}</dd>
          </div>
        </dl>
        <div className="changed-key-comparison">
          <div>
            <span>Trusted</span>
            {event.trustedFingerprintsSha256.map((fingerprint) => (
              <code key={fingerprint}>{fingerprint}</code>
            ))}
          </div>
          <div>
            <span>Received</span>
            <code>{event.receivedFingerprintSha256}</code>
          </div>
        </div>
        <div className="dialog-security-note danger-note">
          <WarningIcon />
          <span>
            Verify the server through another trusted channel before removing
            the saved identity.
          </span>
        </div>
        <div className="dialog-actions">
          <button className="secondary-button" onClick={onClose} type="button">
            Close
          </button>
          <button
            className="connect-button"
            onClick={onOpenKnownHosts}
            type="button"
          >
            Open Known Hosts
          </button>
        </div>
      </section>
    </div>
  );
}

interface AuthenticationChallengeDialogProps {
  challenge: AuthenticationChallengeEvent;
  onDecision(responses: string[] | null): Promise<void>;
}

export function AuthenticationChallengeDialog({
  challenge,
  onDecision,
}: AuthenticationChallengeDialogProps) {
  const [responses, setResponses] = useState(() =>
    challenge.prompts.map(() => ""),
  );

  function clearResponses() {
    setResponses(challenge.prompts.map(() => ""));
  }

  function submit(event: FormEvent) {
    event.preventDefault();
    const submitted = [...responses];
    clearResponses();
    void onDecision(submitted);
  }

  function cancel() {
    clearResponses();
    void onDecision(null);
  }

  return (
    <div className="dialog-backdrop">
      <section
        aria-labelledby="authentication-challenge-title"
        aria-modal="true"
        className="host-key-dialog authentication-dialog"
        role="dialog"
      >
        <div className="dialog-icon">
          <ShieldIcon />
        </div>
        <p className="eyebrow dialog-context-label">
          Additional authentication ·{" "}
          {challenge.hop.kind === "target"
            ? "Target host"
            : `Jump host ${challenge.hop.index}`}
        </p>
        <h2 id="authentication-challenge-title">
          {challenge.name || "Additional authentication"}
        </h2>
        {challenge.instructions && (
          <p className="authentication-instructions">
            {challenge.instructions}
          </p>
        )}
        <dl>
          <div>
            <dt>Host</dt>
            <dd>
              {challenge.host}:{challenge.port}
            </dd>
          </div>
        </dl>
        <form className="authentication-form" onSubmit={submit}>
          {challenge.prompts.map((prompt, index) => (
            <label key={`${challenge.requestId}-${index}`}>
              {prompt.text || `Response ${index + 1}`}
              <input
                autoComplete={prompt.echo ? "off" : "one-time-code"}
                autoFocus={index === 0}
                onChange={(event) =>
                  setResponses((current) =>
                    current.map((value, responseIndex) =>
                      responseIndex === index ? event.target.value : value,
                    ),
                  )
                }
                spellCheck={false}
                type={prompt.echo ? "text" : "password"}
                value={responses[index] ?? ""}
              />
            </label>
          ))}
          <div className="dialog-security-note">
            <ShieldIcon />
            <span>
              Responses belong only to this connection and are cleared after
              submission or cancellation.
            </span>
          </div>
          <div className="dialog-actions">
            <button className="secondary-button" onClick={cancel} type="button">
              Cancel authentication
            </button>
            <button className="connect-button" type="submit">
              Continue
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
