import { useTranslation } from "react-i18next";
import type { AgentSocket } from "../../api/generated/AgentSocket";
import { CopyButton } from "../controls/CopyButton";
import { StatusNote } from "../settings/StatusNote";

type SocketErrorCode = "dir_unsafe" | "second_instance" | "bind_failed" | "bridge_missing";

const ERRORS = {
  dir_unsafe: "agent.connect.error.dir_unsafe",
  second_instance: "agent.connect.error.second_instance",
  bind_failed: "agent.connect.error.bind_failed",
  bridge_missing: "agent.connect.error.bridge_missing",
} as const;

const isKnown = (code: string): code is SocketErrorCode => code in ERRORS;

/**
 * «Как подключить»: путь, фрагмент для Claude Desktop и команда для Claude Code моноширинным шрифтом, у каждого «Копировать».
 * Если подключиться нельзя (папка сокета чужая, второй Kinige, `bind` не удался, нет моста), вместо этого — одна строка
 * ошибки. Конфиги клиентов приложение не правит.
 */
export function AgentConnect({ socket }: { socket: AgentSocket }) {
  const { t } = useTranslation();
  if (socket.error) {
    return (
      <StatusNote tone="error" icon="alert" role="alert">
        {t(isKnown(socket.error) ? ERRORS[socket.error] : ERRORS.bind_failed)}
      </StatusNote>
    );
  }
  return (
    <div className="agent-connect">
      {socket.desktopConfig ? (
        <CodeBlock
          title={t("agent.connect.desktopTitle")}
          text={socket.desktopConfig}
          copyLabel={t("agent.connect.copyDesktop")}
          hint={t("agent.connect.desktopHint")}
        />
      ) : null}
      {socket.codeCommand ? (
        <CodeBlock title={t("agent.connect.codeTitle")} text={socket.codeCommand} copyLabel={t("agent.connect.copyCode")} />
      ) : null}
    </div>
  );
}

function CodeBlock({ title, text, copyLabel, hint }: { title: string; text: string; copyLabel: string; hint?: string }) {
  const { t } = useTranslation();
  return (
    <div className="agent-code">
      <div className="agent-code__head">
        <span className="agent-code__title">{title}</span>
        <CopyButton text={text} label={copyLabel} copiedLabel={t("agent.connect.copied")} failedLabel={t("agent.connect.copyFailed")} />
      </div>
      <pre className="agent-code__text">
        <code>{text}</code>
      </pre>
      {hint ? <p className="agent-code__hint">{hint}</p> : null}
    </div>
  );
}
