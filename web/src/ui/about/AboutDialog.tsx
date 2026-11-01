import { useTranslation } from "react-i18next";
import { Modal } from "../settings/Modal";
import "./about.css";

export interface AboutDialogProps {
  /** Строка сборки от Rust («Kinige 0.5.0», `KinigeApi.appInfo()`): версия берётся из бандла, а не из веб-кода. */
  title: string;
  onClose: () => void;
}

/** «О программе» поверх главного окна (Modal): название с версией и одна строка о приложении. Второго окна ОС нет. */
export function AboutDialog({ title, onClose }: AboutDialogProps) {
  const { t } = useTranslation();
  return (
    <Modal title={t("profile.about")} closeLabel={t("common.close")} onClose={onClose}>
      <div className="about">
        <p className="about__name">{title}</p>
        <p className="about__text">{t("about.tagline")}</p>
      </div>
    </Modal>
  );
}
