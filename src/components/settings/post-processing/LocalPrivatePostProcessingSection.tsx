import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { ChevronDown, ChevronUp, RefreshCcw } from "lucide-react";
import {
  commands,
  type LocalLlmModelInfo,
  type LocalLlmPerformancePreset,
} from "@/bindings";
import { Alert } from "../../ui/Alert";
import { Button } from "../../ui/Button";
import { Dropdown, SettingContainer } from "@/components/ui";
import { Input } from "../../ui/Input";
import { ModelSelect } from "../PostProcessingSettingsApi/ModelSelect";
import { ResetButton } from "../../ui/ResetButton";
import type { ModelOption } from "../PostProcessingSettingsApi/types";
import { useSettings } from "../../../hooks/useSettings";

const LOCAL_LLM_PROGRESS_EVENT = "local-llm-download-progress";

const DEFAULT_LOCAL_CTX = 12288;
const DEFAULT_LOCAL_TEMPERATURE = 0.35;

function localPrivateTierTitle(
  tierId: string,
  translate: (key: string) => string,
): string {
  switch (tierId) {
    case "local_fast":
      return translate("settings.postProcessing.api.localPrivate.model.fast");
    case "local_quality":
      return translate(
        "settings.postProcessing.api.localPrivate.model.quality",
      );
    case "local_apertus":
      return translate(
        "settings.postProcessing.api.localPrivate.model.apertus",
      );
    default:
      return tierId;
  }
}

interface LocalPrivatePostProcessingSectionProps {
  model: string;
  modelOptions: ModelOption[];
  onModelSelect: (value: string) => void;
  onRefreshModels: () => void;
  isFetchingModels: boolean;
  isModelUpdating: boolean;
}

export const LocalPrivatePostProcessingSection: React.FC<
  LocalPrivatePostProcessingSectionProps
> = ({
  model,
  modelOptions,
  onModelSelect,
  onRefreshModels,
  isFetchingModels,
  isModelUpdating,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const [runtimeStatus, setRuntimeStatus] = useState<{
    ready: boolean;
    platform_supported: boolean;
  } | null>(null);
  const [tierDownloaded, setTierDownloaded] = useState<boolean | null>(null);
  const [downloadPct, setDownloadPct] = useState<number | null>(null);
  const [isDownloading, setIsDownloading] = useState(false);
  const [catalog, setCatalog] = useState<LocalLlmModelInfo[]>([]);
  const [showLocalAdvanced, setShowLocalAdvanced] = useState(false);

  const performance: LocalLlmPerformancePreset =
    getSetting("post_process_local_performance") ?? "default";
  const localCtx = getSetting("post_process_local_ctx") ?? DEFAULT_LOCAL_CTX;
  const localMaxTokens = getSetting("post_process_local_max_tokens") ?? 0;
  const localTemperature =
    getSetting("post_process_local_temperature") ?? DEFAULT_LOCAL_TEMPERATURE;
  const localIdleMinutes =
    getSetting("post_process_local_idle_shutdown_minutes") ?? 15;

  const loadStatus = useCallback(async () => {
    const rt = await commands.getLocalLlmRuntimeStatus();
    if (rt.status === "ok") {
      setRuntimeStatus(rt.data);
    }
    if (!model.trim()) {
      setTierDownloaded(null);
      return;
    }
    const down = await commands.isLocalLlmModelDownloaded(model.trim());
    if (down.status === "ok") {
      setTierDownloaded(down.data);
    }
  }, [model]);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  useEffect(() => {
    onRefreshModels();
  }, [onRefreshModels]);

  useEffect(() => {
    void commands.getLocalLlmCatalog().then((result) => {
      if (result.status === "ok") {
        setCatalog(result.data);
      }
    });
  }, []);

  const modelOptionsForSelect = useMemo(() => {
    return modelOptions.map((o) => {
      const entry = catalog.find((c) => c.tier_id === o.value);
      const filename = entry?.filename;
      return {
        value: o.value,
        label: localPrivateTierTitle(o.value, t),
        ...(filename ? { menuDetail: filename } : {}),
      };
    });
  }, [catalog, modelOptions, t]);

  useEffect(() => {
    const unlisten = listen<{
      tier_id: string;
      downloaded: number;
      total: number;
      percentage: number;
    }>(LOCAL_LLM_PROGRESS_EVENT, (event) => {
      if (event.payload.tier_id !== model.trim()) {
        return;
      }
      setDownloadPct(event.payload.percentage);
    });
    return () => {
      void unlisten.then((u) => u());
    };
  }, [model]);

  const handleDownload = async () => {
    const tier = model.trim();
    if (!tier) {
      return;
    }
    setIsDownloading(true);
    setDownloadPct(0);
    try {
      const result = await commands.downloadLocalLlmModel(tier);
      if (result.status === "error") {
        console.error(result.error);
      }
      await loadStatus();
    } finally {
      setIsDownloading(false);
      setDownloadPct(null);
    }
  };

  const performanceOptions = [
    {
      value: "low" as const,
      label: t("settings.postProcessing.api.localPrivate.performance.low"),
    },
    {
      value: "default" as const,
      label: t("settings.postProcessing.api.localPrivate.performance.default"),
    },
    {
      value: "high" as const,
      label: t("settings.postProcessing.api.localPrivate.performance.high"),
    },
  ];

  return (
    <>
      {runtimeStatus && !runtimeStatus.platform_supported ? (
        <Alert variant="error" contained>
          {t("settings.postProcessing.api.localPrivate.unsupportedPlatform")}
        </Alert>
      ) : null}

      <Alert variant="info" contained>
        {t("settings.postProcessing.api.localPrivate.intro")}
      </Alert>

      <SettingContainer
        title={t("settings.postProcessing.api.model.title")}
        description={t(
          "settings.postProcessing.api.localPrivate.model.description",
        )}
        descriptionMode="tooltip"
        layout="stacked"
        grouped={true}
      >
        <div className="flex items-center gap-2">
          <ModelSelect
            value={model}
            options={modelOptionsForSelect}
            disabled={isModelUpdating}
            isLoading={isFetchingModels}
            placeholder={t(
              "settings.postProcessing.api.model.placeholderWithOptions",
            )}
            onSelect={onModelSelect}
            onCreate={onModelSelect}
            onBlur={() => {}}
            className="flex-1 min-w-[380px]"
          />
          <ResetButton
            onClick={onRefreshModels}
            disabled={isFetchingModels}
            ariaLabel={t("settings.postProcessing.api.model.refreshModels")}
            className="flex h-10 w-10 items-center justify-center"
          >
            <RefreshCcw
              className={`h-4 w-4 ${isFetchingModels ? "animate-spin" : ""}`}
            />
          </ResetButton>
        </div>
        {tierDownloaded === false && model.trim() !== "" && (
          <div className="mt-3 space-y-2">
            <Button
              variant="primary"
              size="md"
              onClick={() => void handleDownload()}
              disabled={isDownloading || !runtimeStatus?.platform_supported}
            >
              {t("settings.postProcessing.api.localPrivate.downloadModel")}
            </Button>
            {downloadPct !== null && (
              <div className="w-full max-w-md">
                <div className="h-2 bg-mid-gray/15 rounded overflow-hidden">
                  <div
                    className="h-full bg-primary transition-all"
                    style={{ width: `${Math.min(100, Math.round(downloadPct))}%` }}
                  />
                </div>
                <p className="text-xs text-mid-gray mt-1">
                  {Math.round(downloadPct)}%
                </p>
              </div>
            )}
          </div>
        )}
      </SettingContainer>

      <SettingContainer
        title={t("settings.postProcessing.api.localPrivate.performance.title")}
        description={t(
          "settings.postProcessing.api.localPrivate.performance.description",
        )}
        descriptionMode="tooltip"
        layout="horizontal"
        grouped={true}
      >
        <Dropdown
          selectedValue={performance}
          options={performanceOptions}
          onSelect={(value) => {
            void updateSetting(
              "post_process_local_performance",
              value as LocalLlmPerformancePreset,
            );
          }}
          disabled={isUpdating("post_process_local_performance")}
          className="min-w-[240px]"
        />
      </SettingContainer>

      <div className="px-1">
        <Button
          variant="ghost"
          size="sm"
          type="button"
          aria-expanded={showLocalAdvanced}
          onClick={() => setShowLocalAdvanced(!showLocalAdvanced)}
          className="flex items-center gap-2 text-mid-gray hover:text-logo-primary"
        >
          {showLocalAdvanced ? (
            <ChevronUp className="h-4 w-4 shrink-0" aria-hidden />
          ) : (
            <ChevronDown className="h-4 w-4 shrink-0" aria-hidden />
          )}
          <span>
            {t("settings.postProcessing.api.localPrivate.advanced.toggle")}
          </span>
        </Button>
      </div>

      {showLocalAdvanced ? (
        <>
          <SettingContainer
            title={t(
              "settings.postProcessing.api.localPrivate.inference.ctx.title",
            )}
            description={t(
              "settings.postProcessing.api.localPrivate.inference.ctx.description",
            )}
            descriptionMode="tooltip"
            layout="horizontal"
            grouped={true}
          >
            <Input
              type="number"
              min={2048}
              max={131072}
              step={1024}
              defaultValue={String(localCtx)}
              key={`local-ctx-${localCtx}`}
              disabled={isUpdating("post_process_local_ctx")}
              className="min-w-[140px] max-w-[200px]"
              onBlur={(e) => {
                const parsed = parseInt(e.target.value, 10);
                if (Number.isNaN(parsed)) {
                  return;
                }
                void updateSetting("post_process_local_ctx", parsed);
              }}
            />
          </SettingContainer>

          <SettingContainer
            title={t(
              "settings.postProcessing.api.localPrivate.inference.maxTokens.title",
            )}
            description={t(
              "settings.postProcessing.api.localPrivate.inference.maxTokens.description",
            )}
            descriptionMode="tooltip"
            layout="horizontal"
            grouped={true}
          >
            <Input
              type="number"
              min={0}
              max={131072}
              step={256}
              defaultValue={String(localMaxTokens)}
              key={`local-max-${localMaxTokens}`}
              disabled={isUpdating("post_process_local_max_tokens")}
              className="min-w-[140px] max-w-[200px]"
              onBlur={(e) => {
                const parsed = parseInt(e.target.value, 10);
                if (Number.isNaN(parsed)) {
                  return;
                }
                void updateSetting("post_process_local_max_tokens", parsed);
              }}
            />
          </SettingContainer>

          <SettingContainer
            title={t(
              "settings.postProcessing.api.localPrivate.inference.temperature.title",
            )}
            description={t(
              "settings.postProcessing.api.localPrivate.inference.temperature.description",
            )}
            descriptionMode="tooltip"
            layout="horizontal"
            grouped={true}
          >
            <Input
              type="number"
              min={0}
              max={2}
              step={0.05}
              defaultValue={String(localTemperature)}
              key={`local-temp-${localTemperature}`}
              disabled={isUpdating("post_process_local_temperature")}
              className="min-w-[140px] max-w-[200px]"
              onBlur={(e) => {
                const parsed = parseFloat(e.target.value);
                if (Number.isNaN(parsed)) {
                  return;
                }
                void updateSetting("post_process_local_temperature", parsed);
              }}
            />
          </SettingContainer>

          <SettingContainer
            title={t(
              "settings.postProcessing.api.localPrivate.inference.idleShutdown.title",
            )}
            description={t(
              "settings.postProcessing.api.localPrivate.inference.idleShutdown.description",
            )}
            descriptionMode="tooltip"
            layout="horizontal"
            grouped={true}
          >
            <Input
              type="number"
              min={0}
              max={720}
              step={1}
              defaultValue={String(localIdleMinutes)}
              key={`local-idle-${localIdleMinutes}`}
              disabled={isUpdating("post_process_local_idle_shutdown_minutes")}
              className="min-w-[140px] max-w-[200px]"
              onBlur={(e) => {
                const parsed = parseInt(e.target.value, 10);
                if (Number.isNaN(parsed)) {
                  return;
                }
                void updateSetting(
                  "post_process_local_idle_shutdown_minutes",
                  parsed,
                );
              }}
            />
          </SettingContainer>
        </>
      ) : null}
    </>
  );
};
