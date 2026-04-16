import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown, SettingContainer } from "@/components/ui";
import { useSettings } from "@/hooks/useSettings";

interface PostProcessingTemplatePickerProps {
  grouped?: boolean;
}

export const PostProcessingTemplatePicker: React.FC<
  PostProcessingTemplatePickerProps
> = ({ grouped = false }) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const enabled = getSetting("post_process_enabled") || false;
  const prompts = getSetting("post_process_prompts") || [];
  const selectedPromptId = getSetting("post_process_selected_prompt_id") || "";

  if (!enabled || prompts.length === 0) {
    return null;
  }

  const hasPrompts = prompts.length > 0;

  const handlePromptSelect = (promptId: string | null) => {
    if (!promptId) return;
    updateSetting("post_process_selected_prompt_id", promptId);
  };

  return (
    <SettingContainer
      title={t("settings.postProcessing.prompts.selectedPrompt.title")}
      description={t(
        "settings.postProcessing.prompts.selectedPrompt.description",
      )}
      descriptionMode="tooltip"
      layout="horizontal"
      grouped={grouped}
    >
      <Dropdown
        selectedValue={selectedPromptId || null}
        options={prompts.map((prompt: { id: string; name: string; description?: string | null }) => ({
          value: prompt.id,
          label: prompt.description
            ? `${prompt.name} — ${prompt.description}`
            : prompt.name,
        }))}
        onSelect={(value) => handlePromptSelect(value)}
        placeholder={
          hasPrompts
            ? t("settings.postProcessing.prompts.selectPrompt")
            : t("settings.postProcessing.prompts.noPrompts")
        }
        disabled={isUpdating("post_process_selected_prompt_id")}
        className="w-full min-w-0"
      />
    </SettingContainer>
  );
};

