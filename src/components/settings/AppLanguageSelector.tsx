import React from "react";
import { Trans, useTranslation } from "react-i18next";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { SUPPORTED_LANGUAGES, type SupportedLanguageCode } from "../../i18n";
import { useSettings } from "@/hooks/useSettings";
import { miccyTransComponents } from "@/lib/miccy-trans-components";

interface AppLanguageSelectorProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const AppLanguageSelector: React.FC<AppLanguageSelectorProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t, i18n } = useTranslation();
    const { settings, updateSetting } = useSettings();

    const currentLanguage = (settings?.app_language ||
      i18n.language) as SupportedLanguageCode;

    const languageOptions = SUPPORTED_LANGUAGES.map((lang) => ({
      value: lang.code,
      label: `${lang.nativeName} (${lang.name})`,
    }));

    const handleLanguageChange = (langCode: string) => {
      i18n.changeLanguage(langCode);
      updateSetting("app_language", langCode);
    };

    return (
      <SettingContainer
        title={t("appLanguage.title")}
        description={
          <Trans
            i18nKey="appLanguage.description"
            components={miccyTransComponents}
          />
        }
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <Dropdown
          options={languageOptions}
          selectedValue={currentLanguage}
          onSelect={handleLanguageChange}
        />
      </SettingContainer>
    );
  });

AppLanguageSelector.displayName = "AppLanguageSelector";
