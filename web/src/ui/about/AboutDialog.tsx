import { useTranslation } from "react-i18next";
import { Modal } from "../settings/Modal";
import "./about.css";

export interface AboutDialogProps {
  /** The build line from Rust ("Kinige 0.5.0", `KinigeApi.appInfo()`): the version comes from the bundle, not from the web code. */
  title: string;
  onClose: () => void;
}

/** "About" over the main window (Modal): the name with the version and one line about the app. There is no second OS window. */
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
