import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { ArrowLeft, Loader2 } from "lucide-react";
import { toast } from "sonner";
import { commands, type LocalLlmModelInfo } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { MiccyHeroLogo } from "../branding/MiccyHeroLogo";
import { Button } from "../ui/Button";
import { Alert } from "../ui/Alert";
import Badge from "../ui/Badge";
import { formatModelSize } from "@/lib/utils/format";

const LOCAL_LLM_PROGRESS_EVENT = "local-llm-download-progress";

const TIER_ORDER = ["local_fast", "local_quality", "local_apertus"] as const;

interface StructuringOnboardingProps {
  onComplete: () => void;
}

type Step = 1 | 2;
type LlmPane = "choose" | "localTierPick" | "local" | "ollama";

const choiceCardBase =
  "flex flex-col items-stretch text-start rounded-xl p-4 px-4 border-2 transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-logo-primary/25 active:scale-[0.99] cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed";
const choiceCardIdle =
  "border-mid-gray/20 hover:border-logo-primary/50 hover:bg-logo-primary/5 hover:shadow-lg";

function tierDisplayName(
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

const StructuringOnboarding: React.FC<StructuringOnboardingProps> = ({
  onComplete,
}) => {
  const { t, i18n } = useTranslation();
  const { updateSetting, setPostProcessProvider, updatePostProcessModel } =
    useSettings();
  const [step, setStep] = useState<Step>(1);
  const [llmPane, setLlmPane] = useState<LlmPane>("choose");
  const [platformSupported, setPlatformSupported] = useState<boolean | null>(
    null,
  );
  const [localCatalog, setLocalCatalog] = useState<LocalLlmModelInfo[]>([]);
  const [selectedLocalTier, setSelectedLocalTier] = useState<string>(
    TIER_ORDER[0],
  );
  const [activeDownloadTierId, setActiveDownloadTierId] = useState<
    string | null
  >(null);
  const [downloadPct, setDownloadPct] = useState<number | null>(null);
  const [isLocalDownloadRunning, setIsLocalDownloadRunning] = useState(false);
  const [configureBusy, setConfigureBusy] = useState(false);

  const orderedCatalog = useMemo(() => {
    const allowed = new Set<string>(TIER_ORDER);
    return [...localCatalog]
      .filter((e) => allowed.has(e.tier_id))
      .sort(
        (a, b) =>
          TIER_ORDER.indexOf(a.tier_id as (typeof TIER_ORDER)[number]) -
          TIER_ORDER.indexOf(b.tier_id as (typeof TIER_ORDER)[number]),
      );
  }, [localCatalog]);

  useEffect(() => {
    void commands.getLocalLlmCatalog().then((res) => {
      if (res.status === "ok") {
        setLocalCatalog(res.data);
      }
    });
    void commands.getLocalLlmRuntimeStatus().then((res) => {
      if (res.status === "ok") {
        setPlatformSupported(res.data.platform_supported);
      } else {
        setPlatformSupported(false);
      }
    });
  }, []);

  useEffect(() => {
    const unlisten = listen<{
      tier_id: string;
      downloaded: number;
      total: number;
      percentage: number;
    }>(LOCAL_LLM_PROGRESS_EVENT, (e) => {
      if (
        activeDownloadTierId !== null &&
        e.payload.tier_id === activeDownloadTierId
      ) {
        setDownloadPct(e.payload.percentage);
      }
    });
    return () => {
      void unlisten.then((u) => u());
    };
  }, [activeDownloadTierId]);

  const applyDictationOnly = useCallback(async () => {
    setConfigureBusy(true);
    try {
      await updateSetting("post_process_enabled", false);
      onComplete();
    } finally {
      setConfigureBusy(false);
    }
  }, [onComplete, updateSetting]);

  const goToStructuringSetup = useCallback(() => {
    setStep(2);
    setLlmPane("choose");
  }, []);

  const goBackToStep1 = useCallback(() => {
    setStep(1);
    setLlmPane("choose");
  }, []);

  const openLocalTierPicker = useCallback(() => {
    setSelectedLocalTier(TIER_ORDER[0]);
    setLlmPane("localTierPick");
  }, []);

  const confirmLocalTierAndDownload = useCallback(async () => {
    const tier = selectedLocalTier.trim();
    if (!tier) {
      return;
    }
    setConfigureBusy(true);
    setIsLocalDownloadRunning(true);
    setDownloadPct(0);
    setActiveDownloadTierId(tier);
    try {
      await updateSetting("post_process_enabled", true);
      await setPostProcessProvider("local_private");
      await updatePostProcessModel("local_private", tier);
      setLlmPane("local");
      void commands
        .downloadLocalLlmModel(tier)
        .then((result) => {
          if (result.status === "error") {
            toast.error(
              t("onboarding.structuring.errors.downloadLlm", {
                error: result.error,
              }),
            );
          }
        })
        .finally(() => {
          setIsLocalDownloadRunning(false);
          setDownloadPct(null);
          setActiveDownloadTierId(null);
        });
    } catch (e) {
      toast.error(
        t("onboarding.structuring.errors.downloadLlm", {
          error: String(e),
        }),
      );
      setIsLocalDownloadRunning(false);
      setDownloadPct(null);
      setActiveDownloadTierId(null);
    } finally {
      setConfigureBusy(false);
    }
  }, [
    selectedLocalTier,
    setPostProcessProvider,
    t,
    updatePostProcessModel,
    updateSetting,
  ]);

  const chooseOllama = useCallback(async () => {
    setConfigureBusy(true);
    try {
      await updateSetting("post_process_enabled", true);
      await setPostProcessProvider("custom");
      setLlmPane("ollama");
    } catch (e) {
      toast.error(
        t("onboarding.structuring.errors.configure", { error: String(e) }),
      );
    } finally {
      setConfigureBusy(false);
    }
  }, [setPostProcessProvider, t, updateSetting]);

  const handleContinueToApp = useCallback(() => {
    onComplete();
  }, [onComplete]);

  const handleStep2BackRow = useCallback(() => {
    if (llmPane === "choose") {
      goBackToStep1();
      return;
    }
    if (llmPane === "localTierPick" || llmPane === "ollama") {
      setLlmPane("choose");
      return;
    }
    if (llmPane === "local") {
      setLlmPane("localTierPick");
    }
  }, [goBackToStep1, llmPane]);

  const showStep2BackButton =
    step === 2 &&
    !(llmPane === "local" && (isLocalDownloadRunning || downloadPct !== null));

  const step2BackLabel =
    llmPane === "local"
      ? t("onboarding.structuring.backToTierPick")
      : t("onboarding.structuring.back");

  const renderStep1 = () => (
    <div className="max-w-[560px] w-full mx-auto flex flex-col gap-4">
      <div className="text-center mb-1">
        <h2 className="text-xl font-semibold text-text mb-2">
          {t("onboarding.structuring.step1.title")}
        </h2>
        <p className="text-sm text-text/70">
          {t("onboarding.structuring.step1.subtitle")}
        </p>
      </div>
      <button
        type="button"
        disabled={configureBusy}
        onClick={() => void applyDictationOnly()}
        className={`${choiceCardBase} ${choiceCardIdle}`}
      >
        <h3 className="text-lg font-semibold text-text mb-1">
          {t("onboarding.structuring.step1.dictationOnly.title")}
        </h3>
        <p className="text-sm text-text/65 leading-relaxed">
          {t("onboarding.structuring.step1.dictationOnly.description")}
        </p>
      </button>
      <button
        type="button"
        disabled={configureBusy}
        onClick={goToStructuringSetup}
        className={`${choiceCardBase} border-2 border-logo-primary/25 bg-logo-primary/5 hover:border-logo-primary/40 hover:bg-logo-primary/8 hover:shadow-lg`}
      >
        <h3 className="text-lg font-semibold text-text mb-1">
          {t("onboarding.structuring.step1.withStructuring.title")}
        </h3>
        <p className="text-sm text-text/65 leading-relaxed">
          {t("onboarding.structuring.step1.withStructuring.description")}
        </p>
      </button>
    </div>
  );

  const renderLlmChoose = () => {
    if (platformSupported === null) {
      return (
        <div className="flex justify-center py-12">
          <Loader2 className="w-8 h-8 animate-spin text-text/45" />
        </div>
      );
    }
    const showLocal = platformSupported;
    return (
      <div className="max-w-[560px] w-full mx-auto flex flex-col gap-4">
        {!showLocal ? (
          <Alert variant="info" contained>
            {t("onboarding.structuring.step2.localUnsupported")}
          </Alert>
        ) : null}
        {showLocal ? (
          <button
            type="button"
            disabled={configureBusy}
            onClick={openLocalTierPicker}
            className={`${choiceCardBase} ${choiceCardIdle}`}
          >
            <h3 className="text-lg font-semibold text-text mb-1">
              {t("onboarding.structuring.step2.local.title")}
            </h3>
            <p className="text-sm text-text/65 leading-relaxed">
              {t("onboarding.structuring.step2.local.lead")}
            </p>
          </button>
        ) : null}
        <button
          type="button"
          disabled={configureBusy}
          onClick={() => void chooseOllama()}
          className={`${choiceCardBase} ${choiceCardIdle}`}
        >
          <h3 className="text-lg font-semibold text-text mb-1">
            {t("onboarding.structuring.step2.ollama.title")}
          </h3>
          <p className="text-sm text-text/65 leading-relaxed">
            {t("onboarding.structuring.step2.ollama.description")}
          </p>
        </button>
      </div>
    );
  };

  const renderLocalTierPick = () => (
    <div className="max-w-[560px] w-full mx-auto flex flex-col gap-4 min-w-0">
      <div className="text-center mb-1">
        <h2 className="text-xl font-semibold text-text mb-2">
          {t("onboarding.structuring.localTierPick.title")}
        </h2>
        <p className="text-sm text-text/70">
          {t("onboarding.structuring.localTierPick.subtitle")}
        </p>
      </div>
      {orderedCatalog.length === 0 ? (
        <Alert variant="info" contained>
          {t("onboarding.structuring.localTierPick.catalogUnavailable")}
        </Alert>
      ) : null}
      <div className="flex flex-col gap-3">
        {orderedCatalog.map((entry) => {
          const isSelected = selectedLocalTier === entry.tier_id;
          const badgeKey = `onboarding.structuring.localTierPick.tiers.${entry.tier_id}.badge`;
          const hintKey = `onboarding.structuring.localTierPick.tiers.${entry.tier_id}.hint`;
          const hasBadge = i18n.exists(badgeKey);
          return (
            <button
              key={entry.tier_id}
              type="button"
              disabled={configureBusy}
              onClick={() => setSelectedLocalTier(entry.tier_id)}
              className={`${choiceCardBase} text-start ${
                isSelected
                  ? "border-logo-primary ring-2 ring-logo-primary/25 bg-logo-primary/5"
                  : choiceCardIdle
              }`}
            >
              <div className="flex flex-wrap items-center gap-2 mb-1">
                <h3 className="text-lg font-semibold text-text">
                  {tierDisplayName(entry.tier_id, t)}
                </h3>
                {hasBadge ? (
                  <Badge variant="primary" className="text-white shrink-0">
                    {t(badgeKey)}
                  </Badge>
                ) : null}
                <span className="text-xs text-text/55 tabular-nums ms-auto">
                  {formatModelSize(Number(entry.size_mb))}
                </span>
              </div>
              <p className="text-sm text-text/65 leading-relaxed">
                {t(hintKey)}
              </p>
            </button>
          );
        })}
      </div>
      <Button
        type="button"
        variant="primary"
        size="lg"
        className="w-full"
        disabled={
          configureBusy ||
          !selectedLocalTier.trim() ||
          orderedCatalog.length === 0
        }
        onClick={() => void confirmLocalTierAndDownload()}
      >
        {t("onboarding.structuring.localTierPick.downloadCta")}
      </Button>
    </div>
  );

  const renderLlmLocalPane = () => (
    <div className="max-w-[560px] w-full mx-auto flex flex-col gap-4">
      <Alert variant="info" contained>
        {t("onboarding.structuring.localPane.intro")}
      </Alert>
      {isLocalDownloadRunning || downloadPct !== null ? (
        <div className="rounded-lg border border-mid-gray/20 p-4 bg-mid-gray/5">
          <p className="text-sm font-medium text-text mb-2">
            {t("onboarding.structuring.localPane.progressTitle")}
          </p>
          {downloadPct !== null ? (
            <>
              <div className="h-2 rounded-full bg-mid-gray/20 overflow-hidden mb-1">
                <div
                  className="h-full bg-logo-primary transition-[width] duration-300"
                  style={{ width: `${Math.min(100, Math.round(downloadPct))}%` }}
                />
              </div>
              <p className="text-xs text-text/60 tabular-nums">
                {t("onboarding.structuring.localPane.percent", {
                  value: Math.round(downloadPct),
                })}
              </p>
            </>
          ) : (
            <p className="text-xs text-text/60">
              {t("onboarding.structuring.localPane.starting")}
            </p>
          )}
        </div>
      ) : null}
      <Button
        type="button"
        variant="primary"
        size="lg"
        className="w-full"
        onClick={handleContinueToApp}
      >
        {t("onboarding.structuring.continue")}
      </Button>
      <p className="text-xs text-center text-text/55">
        {t("onboarding.structuring.localPane.footerHint")}
      </p>
    </div>
  );

  const renderLlmOllamaPane = () => (
    <div className="max-w-[560px] w-full mx-auto flex flex-col gap-4">
      <Alert variant="info" contained>
        {t("onboarding.structuring.ollamaPane.hint")}
      </Alert>
      <Button
        type="button"
        variant="primary"
        size="lg"
        className="w-full"
        onClick={handleContinueToApp}
      >
        {t("onboarding.structuring.continue")}
      </Button>
    </div>
  );

  return (
    <div className="h-screen w-screen flex flex-col p-6 gap-4 inset-0 bg-background overflow-y-auto">
      <div className="flex flex-col items-center gap-2 shrink-0">
        <MiccyHeroLogo width={180} />
      </div>
      <div className="flex-1 flex flex-col min-h-0 w-full max-w-2xl mx-auto">
        {step === 2 && showStep2BackButton ? (
          <div className="mb-3">
            <button
              type="button"
              onClick={handleStep2BackRow}
              className="inline-flex items-center gap-1.5 text-sm text-text/70 hover:text-text transition-colors"
            >
              <ArrowLeft className="w-4 h-4" aria-hidden />
              {step2BackLabel}
            </button>
          </div>
        ) : null}
        {step === 2 && llmPane !== "localTierPick" ? (
          <div className="text-center mb-4">
            <h2 className="text-xl font-semibold text-text mb-2">
              {t("onboarding.structuring.step2.title")}
            </h2>
            <p className="text-sm text-text/70">
              {t("onboarding.structuring.step2.subtitle")}
            </p>
          </div>
        ) : null}
        {step === 1 ? renderStep1() : null}
        {step === 2 && llmPane === "choose" ? renderLlmChoose() : null}
        {step === 2 && llmPane === "localTierPick" ? renderLocalTierPick() : null}
        {step === 2 && llmPane === "local" ? renderLlmLocalPane() : null}
        {step === 2 && llmPane === "ollama" ? renderLlmOllamaPane() : null}
      </div>
    </div>
  );
};

export default StructuringOnboarding;
