import { useEffect, useMemo, useState } from "react";
import { Icon } from "./components/Icon";
import { ScreenCanvas } from "./components/ScreenCanvas";
import {
  FLOWS,
  FLOW_BY_ID,
  NEXT_SCREEN,
  PRIMARY_JOURNEY,
  REVIEW_STATUS_LABEL,
  SCREENS,
  SCREEN_BY_ID,
  type FlowDefinition,
  type Platform,
  type PlatformMode,
  type PrototypeTheme,
  type ReviewEntry,
  type ReviewMap,
  type ReviewStatus,
  type ScreenDefinition,
  type ScreenId,
} from "./model";

type ReviewView = "overview" | "prototype";

const REVIEW_STORAGE_KEY = "anyssh-design-review-v1";
const EMPTY_REVIEW: ReviewEntry = { status: "pending", note: "" };

export function App() {
  const [view, setView] = useState<ReviewView>("overview");
  const [selectedScreenId, setSelectedScreenId] = useState<ScreenId>("welcome");
  const [platformMode, setPlatformMode] = useState<PlatformMode>("compare");
  const [theme, setTheme] = useState<PrototypeTheme>("light");
  const [reviews, setReviews] = useState<ReviewMap>(loadReviews);
  const [mobileInspectorOpen, setMobileInspectorOpen] = useState(false);

  useEffect(() => {
    window.localStorage.setItem(REVIEW_STORAGE_KEY, JSON.stringify(reviews));
  }, [reviews]);

  const selectedScreen = SCREEN_BY_ID[selectedScreenId];
  const currentReview = reviews[selectedScreenId] ?? EMPTY_REVIEW;
  const reviewCounts = useMemo(() => countReviews(reviews), [reviews]);

  function selectScreen(screenId: ScreenId, nextView: ReviewView = view) {
    setSelectedScreenId(screenId);
    setView(nextView);
    if (window.innerWidth < 980) setMobileInspectorOpen(false);
  }

  function updateReview(update: Partial<ReviewEntry>) {
    setReviews((current) => ({
      ...current,
      [selectedScreenId]: {
        ...(current[selectedScreenId] ?? EMPTY_REVIEW),
        ...update,
      },
    }));
  }

  function resetSelectedReview() {
    setReviews((current) => {
      const next = { ...current };
      delete next[selectedScreenId];
      return next;
    });
  }

  function resetAllReviews() {
    setReviews({});
    window.localStorage.removeItem(REVIEW_STORAGE_KEY);
  }

  return (
    <div className={`review-app review-theme-${theme}`}>
      <ReviewTopbar
        platformMode={platformMode}
        setPlatformMode={setPlatformMode}
        setTheme={setTheme}
        theme={theme}
      />
      <div className="review-layout">
        <ReviewSidebar
          reviews={reviews}
          selectedScreenId={selectedScreenId}
          selectScreen={selectScreen}
          setView={setView}
          view={view}
        />
        <main className="review-main">
          {view === "overview" ? (
            <Overview
              platformMode={platformMode}
              reviewCounts={reviewCounts}
              reviews={reviews}
              selectScreen={selectScreen}
              theme={theme}
            />
          ) : (
            <PrototypeReview
              platformMode={platformMode}
              screenId={selectedScreenId}
              selectScreen={selectScreen}
              theme={theme}
            />
          )}
        </main>
        <ReviewInspector
          mobileOpen={mobileInspectorOpen}
          onCloseMobile={() => setMobileInspectorOpen(false)}
          onReset={resetSelectedReview}
          review={currentReview}
          screen={selectedScreen}
          updateReview={updateReview}
        />
      </div>
      {mobileInspectorOpen && (
        <button
          aria-label="关闭评审面板"
          className="review-inspector-backdrop"
          onClick={() => setMobileInspectorOpen(false)}
          type="button"
        />
      )}
      <button
        aria-label="打开当前界面的评审面板"
        className="mobile-review-fab"
        onClick={() => setMobileInspectorOpen(true)}
        type="button"
      >
        <Icon name="edit" />
        评审
      </button>
      <ReviewFooter
        approved={reviewCounts.approved}
        changes={reviewCounts.changes}
        onResetAll={resetAllReviews}
        total={SCREENS.length}
      />
    </div>
  );
}

function ReviewTopbar({
  platformMode,
  setPlatformMode,
  setTheme,
  theme,
}: {
  platformMode: PlatformMode;
  setPlatformMode(mode: PlatformMode): void;
  setTheme(theme: PrototypeTheme): void;
  theme: PrototypeTheme;
}) {
  return (
    <header className="review-topbar">
      <div className="review-brand">
        <span>
          <Icon name="terminal" />
        </span>
        <div>
          <strong>AnySSH 产品设计评审</strong>
          <small>Linux + Android · Material Design 3</small>
        </div>
      </div>
      <div className="review-topbar-controls">
        <div
          aria-label="预览平台"
          className="review-segmented-control"
          role="group"
        >
          <button
            aria-pressed={platformMode === "compare"}
            className={platformMode === "compare" ? "active" : ""}
            onClick={() => setPlatformMode("compare")}
            type="button"
          >
            <Icon name="compare" />
            双端
          </button>
          <button
            aria-pressed={platformMode === "linux"}
            className={platformMode === "linux" ? "active" : ""}
            onClick={() => setPlatformMode("linux")}
            type="button"
          >
            <Icon name="desktop" />
            Linux
          </button>
          <button
            aria-pressed={platformMode === "android"}
            className={platformMode === "android" ? "active" : ""}
            onClick={() => setPlatformMode("android")}
            type="button"
          >
            <Icon name="android" />
            Android
          </button>
        </div>
        <button
          aria-label={theme === "light" ? "切换为深色预览" : "切换为浅色预览"}
          className="review-theme-toggle"
          onClick={() => setTheme(theme === "light" ? "dark" : "light")}
          type="button"
        >
          <Icon name={theme === "light" ? "moon" : "sun"} />
          <span>{theme === "light" ? "深色" : "浅色"}</span>
        </button>
      </div>
    </header>
  );
}

function ReviewSidebar({
  reviews,
  selectedScreenId,
  selectScreen,
  setView,
  view,
}: {
  reviews: ReviewMap;
  selectedScreenId: ScreenId;
  selectScreen(screenId: ScreenId, nextView?: ReviewView): void;
  setView(view: ReviewView): void;
  view: ReviewView;
}) {
  return (
    <aside className="review-sidebar">
      <div className="review-mode-switch">
        <button
          className={view === "overview" ? "active" : ""}
          onClick={() => setView("overview")}
          type="button"
        >
          <Icon name="grid" />
          <span>
            <strong>界面总览</strong>
            <small>一次查看全部页面</small>
          </span>
        </button>
        <button
          className={view === "prototype" ? "active" : ""}
          onClick={() => setView("prototype")}
          type="button"
        >
          <Icon name="arrow" />
          <span>
            <strong>交互流程</strong>
            <small>像真实应用一样点击</small>
          </span>
        </button>
      </div>
      <div className="review-sidebar-label">核心用户旅程</div>
      <nav className="review-flow-nav">
        {FLOWS.map((flow) => {
          const reviewed = flow.screenIds.filter(
            (screenId) =>
              reviews[screenId]?.status &&
              reviews[screenId]?.status !== "pending",
          ).length;
          const active = flow.screenIds.includes(selectedScreenId);
          return (
            <button
              className={active ? "active" : ""}
              key={flow.id}
              onClick={() =>
                selectScreen(
                  flow.screenIds[0],
                  view === "overview" ? view : "prototype",
                )
              }
              type="button"
            >
              <span className="flow-nav-number">{flow.number}</span>
              <span>
                <strong>{flow.title}</strong>
                <small>
                  {reviewed}/{flow.screenIds.length} 已评审
                </small>
              </span>
              <Icon name="chevron" />
            </button>
          );
        })}
      </nav>
      <div className="review-sidebar-note">
        <Icon name="info" />
        <div>
          <strong>这是评审原型</strong>
          <span>使用模拟数据，不连接网络、SSH 或真实保险库。</span>
        </div>
      </div>
    </aside>
  );
}

function Overview({
  platformMode,
  reviewCounts,
  reviews,
  selectScreen,
  theme,
}: {
  platformMode: PlatformMode;
  reviewCounts: ReturnType<typeof countReviews>;
  reviews: ReviewMap;
  selectScreen(screenId: ScreenId, nextView?: ReviewView): void;
  theme: PrototypeTheme;
}) {
  const reviewed = reviewCounts.approved + reviewCounts.changes;
  const progress = Math.round((reviewed / SCREENS.length) * 100);

  return (
    <div className="overview-page">
      <section className="review-hero">
        <div className="review-hero-copy">
          <span className="review-eyebrow">PRODUCT DIRECTION / V1</span>
          <h1>先评审体验，再决定怎样实现。</h1>
          <p>
            这份原型把 Linux 与 Android
            放在同一套产品语言中，同时保留桌面信息密度与移动端触控习惯。
          </p>
          <div className="review-hero-tags">
            <span>Material Design 3</span>
            <span>中文优先</span>
            <span>现代专业</span>
            <span>Mock Data Only</span>
          </div>
        </div>
        <div className="review-progress-card">
          <div
            className="progress-ring"
            style={{ "--progress": progress } as React.CSSProperties}
          >
            <span>{progress}%</span>
          </div>
          <div>
            <span>总体评审进度</span>
            <strong>
              {reviewed} / {SCREENS.length} 个界面
            </strong>
            <small>
              {reviewCounts.approved} 个通过 · {reviewCounts.changes} 个待修改
            </small>
          </div>
        </div>
      </section>

      <section className="journey-overview">
        <div className="review-section-heading">
          <div>
            <span className="review-eyebrow">PRIMARY JOURNEY</span>
            <h2>首次使用到成功连接</h2>
          </div>
          <p>建议优先评审这条最重要的纵向流程。</p>
        </div>
        <div className="journey-track">
          {PRIMARY_JOURNEY.map((screenId, index) => {
            const screen = SCREEN_BY_ID[screenId];
            const status = reviews[screenId]?.status ?? "pending";
            return (
              <div className="journey-step-wrap" key={screenId}>
                <button
                  className={`journey-step status-${status}`}
                  onClick={() => selectScreen(screenId, "prototype")}
                  type="button"
                >
                  <span>{String(index + 1).padStart(2, "0")}</span>
                  <strong>{screen.shortTitle}</strong>
                  <small>{REVIEW_STATUS_LABEL[status]}</small>
                </button>
                {index < PRIMARY_JOURNEY.length - 1 && (
                  <Icon className="journey-arrow" name="arrow" />
                )}
              </div>
            );
          })}
        </div>
      </section>

      {FLOWS.map((flow) => (
        <FlowOverview
          flow={flow}
          key={flow.id}
          platformMode={platformMode}
          reviews={reviews}
          selectScreen={selectScreen}
          theme={theme}
        />
      ))}
    </div>
  );
}

function FlowOverview({
  flow,
  platformMode,
  reviews,
  selectScreen,
  theme,
}: {
  flow: FlowDefinition;
  platformMode: PlatformMode;
  reviews: ReviewMap;
  selectScreen(screenId: ScreenId, nextView?: ReviewView): void;
  theme: PrototypeTheme;
}) {
  return (
    <section className="flow-overview-section">
      <div className="flow-overview-heading">
        <span>{flow.number}</span>
        <div>
          <h2>{flow.title}</h2>
          <p>{flow.summary}</p>
        </div>
      </div>
      <div className="screen-gallery">
        {flow.screenIds.map((screenId) => {
          const screen = SCREEN_BY_ID[screenId];
          const status = reviews[screenId]?.status ?? "pending";
          return (
            <article className="screen-review-card" key={screenId}>
              <div className="screen-card-header">
                <div>
                  <span>{screen.shortTitle}</span>
                  <h3>{screen.title}</h3>
                </div>
                <ReviewStatusBadge status={status} />
              </div>
              <ScreenThumbnail
                platformMode={platformMode}
                screenId={screenId}
                theme={theme}
              />
              <p>{screen.description}</p>
              <button
                className="screen-card-action"
                onClick={() => selectScreen(screenId, "prototype")}
                type="button"
              >
                打开并评审
                <Icon name="arrow" />
              </button>
            </article>
          );
        })}
      </div>
    </section>
  );
}

function ScreenThumbnail({
  platformMode,
  screenId,
  theme,
}: {
  platformMode: PlatformMode;
  screenId: ScreenId;
  theme: PrototypeTheme;
}) {
  const platforms: Platform[] =
    platformMode === "compare" ? ["linux", "android"] : [platformMode];

  return (
    <div
      className={`screen-thumbnail screen-thumbnail-${platformMode}`}
      data-testid={`thumbnail-${screenId}`}
    >
      {platforms.map((platform) => (
        <div
          className={`thumbnail-platform thumbnail-platform-${platform}`}
          key={platform}
        >
          <div className="thumbnail-label">
            <Icon name={platform === "linux" ? "desktop" : "android"} />
            {platform === "linux" ? "Linux" : "Android"}
          </div>
          <div className={`thumbnail-scaler thumbnail-scaler-${platform}`}>
            <ScreenCanvas
              interactive={false}
              platform={platform}
              screenId={screenId}
              theme={theme}
            />
          </div>
        </div>
      ))}
    </div>
  );
}

function PrototypeReview({
  platformMode,
  screenId,
  selectScreen,
  theme,
}: {
  platformMode: PlatformMode;
  screenId: ScreenId;
  selectScreen(screenId: ScreenId, nextView?: ReviewView): void;
  theme: PrototypeTheme;
}) {
  const screen = SCREEN_BY_ID[screenId];
  const flow = FLOW_BY_ID[screen.flowId];
  const globalIndex = SCREENS.findIndex((item) => item.id === screenId);
  const previous = globalIndex > 0 ? SCREENS[globalIndex - 1].id : null;
  const next =
    NEXT_SCREEN[screenId] ??
    (globalIndex < SCREENS.length - 1 ? SCREENS[globalIndex + 1].id : null);
  const platforms: Platform[] =
    platformMode === "compare" ? ["linux", "android"] : [platformMode];

  return (
    <div className="prototype-review-page">
      <div className="prototype-review-heading">
        <div>
          <span className="review-eyebrow">
            {flow.number} / {flow.title}
          </span>
          <h1>{screen.title}</h1>
          <p>{screen.description}</p>
        </div>
        <div className="prototype-position">
          <strong>
            {globalIndex + 1} / {SCREENS.length}
          </strong>
          <span>界面</span>
        </div>
      </div>

      <div className="prototype-flow-steps">
        {flow.screenIds.map((flowScreenId, index) => (
          <button
            className={flowScreenId === screenId ? "active" : ""}
            key={flowScreenId}
            onClick={() => selectScreen(flowScreenId, "prototype")}
            type="button"
          >
            <span>{index + 1}</span>
            {SCREEN_BY_ID[flowScreenId].shortTitle}
          </button>
        ))}
      </div>

      <div
        className={`prototype-stage prototype-stage-${platformMode}`}
        data-testid="prototype-stage"
      >
        {platforms.map((platform) => (
          <div
            className={`prototype-device-column prototype-device-${platform}`}
            key={platform}
          >
            <div className="prototype-device-label">
              <span>
                <Icon name={platform === "linux" ? "desktop" : "android"} />
                {platform === "linux" ? "Linux Desktop" : "Android Mobile"}
              </span>
              <small>
                {platform === "linux"
                  ? "1280 × 800 设计基线"
                  : "360 × 800 设计基线"}
              </small>
            </div>
            <div className="prototype-device-surface">
              <ScreenCanvas
                onNavigate={(target) => selectScreen(target, "prototype")}
                platform={platform}
                screenId={screenId}
                theme={theme}
              />
            </div>
          </div>
        ))}
      </div>

      <div className="prototype-navigation">
        <button
          disabled={!previous}
          onClick={() => previous && selectScreen(previous, "prototype")}
          type="button"
        >
          <Icon name="back" />
          <span>
            <small>上一个界面</small>
            <strong>
              {previous ? SCREEN_BY_ID[previous].shortTitle : "已经是第一个"}
            </strong>
          </span>
        </button>
        <div>
          <span>可以直接点击设备中的主按钮体验流程</span>
        </div>
        <button
          disabled={!next}
          onClick={() => next && selectScreen(next, "prototype")}
          type="button"
        >
          <span>
            <small>下一个界面</small>
            <strong>
              {next ? SCREEN_BY_ID[next].shortTitle : "已经是最后一个"}
            </strong>
          </span>
          <Icon name="arrow" />
        </button>
      </div>
    </div>
  );
}

function ReviewInspector({
  mobileOpen,
  onCloseMobile,
  onReset,
  review,
  screen,
  updateReview,
}: {
  mobileOpen: boolean;
  onCloseMobile(): void;
  onReset(): void;
  review: ReviewEntry;
  screen: ScreenDefinition;
  updateReview(update: Partial<ReviewEntry>): void;
}) {
  return (
    <aside
      className={`review-inspector ${mobileOpen ? "mobile-open" : ""}`}
      aria-label="当前界面评审"
    >
      <div className="inspector-mobile-header">
        <strong>界面评审</strong>
        <button onClick={onCloseMobile} type="button">
          关闭
        </button>
      </div>
      <div className="inspector-heading">
        <span className="review-eyebrow">SCREEN REVIEW</span>
        <h2>{screen.title}</h2>
        <p>{screen.purpose}</p>
      </div>
      <div className="inspector-section">
        <span className="inspector-label">评审状态</span>
        <div className="review-status-control" role="group">
          {(
            [
              ["pending", "clock"],
              ["approved", "check"],
              ["changes", "edit"],
            ] as const
          ).map(([status, icon]) => (
            <button
              aria-pressed={review.status === status}
              className={`status-${status} ${
                review.status === status ? "active" : ""
              }`}
              key={status}
              onClick={() => updateReview({ status })}
              type="button"
            >
              <Icon name={icon === "clock" ? "more" : icon} />
              {REVIEW_STATUS_LABEL[status]}
            </button>
          ))}
        </div>
      </div>
      <div className="inspector-section">
        <label className="inspector-label" htmlFor="review-note">
          评审备注
        </label>
        <textarea
          id="review-note"
          onChange={(event) => updateReview({ note: event.target.value })}
          placeholder="例如：Android 的主按钮再大一点；标题改成……"
          value={review.note}
        />
        <span className="inspector-helper">
          备注只保存在当前浏览器，不会上传。
        </span>
      </div>
      <div className="inspector-section">
        <span className="inspector-label">本页检查重点</span>
        <ul className="review-checklist">
          {screen.checkpoints.map((checkpoint) => (
            <li key={checkpoint}>
              <Icon name="check" />
              <span>{checkpoint}</span>
            </li>
          ))}
        </ul>
      </div>
      <div className="inspector-section inspector-context">
        <span className="inspector-label">设计说明</span>
        <dl>
          <div>
            <dt>所属流程</dt>
            <dd>{FLOW_BY_ID[screen.flowId].title}</dd>
          </div>
          <div>
            <dt>目标平台</dt>
            <dd>Linux + Android</dd>
          </div>
          <div>
            <dt>数据</dt>
            <dd>仅模拟数据</dd>
          </div>
        </dl>
      </div>
      <button className="inspector-reset" onClick={onReset} type="button">
        清除此页评审记录
      </button>
    </aside>
  );
}

function ReviewStatusBadge({ status }: { status: ReviewStatus }) {
  return (
    <span className={`review-status-badge status-${status}`}>
      <span />
      {REVIEW_STATUS_LABEL[status]}
    </span>
  );
}

function ReviewFooter({
  approved,
  changes,
  onResetAll,
  total,
}: {
  approved: number;
  changes: number;
  onResetAll(): void;
  total: number;
}) {
  return (
    <footer className="review-footer">
      <span>
        AnySSH Design Review · 所有界面均为评审用 Mock，不代表最终实现。
      </span>
      <div>
        <strong>
          {approved} 通过 · {changes} 待修改 · {total} 总计
        </strong>
        <button onClick={onResetAll} type="button">
          清空全部评审
        </button>
      </div>
    </footer>
  );
}

function loadReviews(): ReviewMap {
  try {
    const raw = window.localStorage.getItem(REVIEW_STORAGE_KEY);
    if (!raw) return {};
    return JSON.parse(raw) as ReviewMap;
  } catch {
    return {};
  }
}

function countReviews(reviews: ReviewMap) {
  return SCREENS.reduce(
    (counts, screen) => {
      const status = reviews[screen.id]?.status ?? "pending";
      counts[status] += 1;
      return counts;
    },
    { pending: 0, approved: 0, changes: 0 } as Record<ReviewStatus, number>,
  );
}
