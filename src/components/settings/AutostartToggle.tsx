import React from "react";
import { Trans, useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { miccyTransComponents } from "@/lib/miccy-trans-components";
import { useSettings } from "../../hooks/useSettings";

interface AutostartToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const AutostartToggle: React.FC<AutostartToggleProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const autostartEnabled = getSetting("autostart_enabled") ?? false;

    return (
      <ToggleSwitch
        checked={autostartEnabled}
        onChange={(enabled) => updateSetting("autostart_enabled", enabled)}
        isUpdating={isUpdating("autostart_enabled")}
        label={t("settings.advanced.autostart.label")}
        description={
          <Trans
            i18nKey="settings.advanced.autostart.description"
            components={miccyTransComponents}
          />
        }
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
