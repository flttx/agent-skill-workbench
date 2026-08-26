import {
  animate,
  createDrawable,
  createScope,
  createTimeline,
  stagger,
  utils,
} from "animejs";

type MotionElement = HTMLElement | SVGElement;

export type WorkbenchMotion = {
  enterShell: () => void;
  enterView: () => void;
  animateWorkspaceRows: () => void;
  animatePortal: (open: boolean, contentSelector: string) => void;
  dispose: () => void;
};

const collect = <T extends MotionElement>(root: HTMLElement, selector: string) =>
  Array.from(root.querySelectorAll<T>(selector));

const visible = (elements: MotionElement[]) =>
  elements.filter((element) => {
    const style = window.getComputedStyle(element);
    return style.display !== "none" && style.visibility !== "hidden";
  });

export function createWorkbenchMotion(root: HTMLElement): WorkbenchMotion {
  const scope = createScope({
    root,
    mediaQueries: {
      reduceMotion: "(prefers-reduced-motion: reduce)",
      compact: "(max-width: 760px)",
    },
  });

  const isReduced = () => Boolean(scope.matches.reduceMotion);
  const isCompact = () => Boolean(scope.matches.compact);
  const duration = (value: number) =>
    isReduced() ? 1 : isCompact() ? Math.round(value * 0.82) : value;
  const ease = () => (isReduced() ? "linear" : "outExpo");

  const enter = (
    elements: MotionElement[],
    options: {
      delay?: number;
      duration?: number;
      y?: number;
      scale?: number;
      staggerDelay?: number;
    } = {},
  ) => {
    const targets = visible(elements);
    if (!targets.length) return;
    const initialY = isCompact() ? Math.min(options.y ?? 16, 10) : options.y ?? 16;
    const initialScale = options.scale ?? 0.985;
    const targetDuration = options.duration ?? 560;
    scope.execute(() => {
      utils.set(targets, {
        opacity: 0,
        y: initialY,
        scale: initialScale,
      });
      return animate(targets, {
        opacity: 1,
        y: 0,
        scale: 1,
        delay: options.staggerDelay
          ? stagger(options.staggerDelay)
          : options.delay ?? 0,
        duration: duration(targetDuration),
        ease: ease(),
      });
    });
  };

  const enterShell = () => {
    const timeline = createTimeline({
      defaults: {
        duration: duration(640),
        ease: ease(),
      },
    });
    const brand = collect<HTMLElement>(root, ".brand");
    const workspaceHeading = collect<HTMLElement>(
      root,
      ".sidebar-section-heading",
    );
    const workspaceRows = collect<HTMLElement>(root, ".workspace-list > *");
    const navigation = collect<HTMLElement>(root, ".global-nav > button");
    const sidebarBottom = collect<HTMLElement>(root, ".sidebar-bottom");
    const topbar = collect<HTMLElement>(root, ".workbench-topbar");

    scope.execute(() => {
      const allTargets = [
        ...brand,
        ...workspaceHeading,
        ...workspaceRows,
        ...navigation,
        ...sidebarBottom,
        ...topbar,
      ];
      utils.set(allTargets, { opacity: 0, y: 14 });
      if (brand.length) timeline.add(brand, { opacity: 1, y: 0 }, 0);
      if (workspaceHeading.length) {
        timeline.add(workspaceHeading, { opacity: 1, y: 0 }, 90);
      }
      if (workspaceRows.length) {
        timeline.add(
          workspaceRows,
          { opacity: 1, y: 0, delay: stagger(55) },
          150,
        );
      }
      if (navigation.length) {
        timeline.add(
          navigation,
          { opacity: 1, y: 0, delay: stagger(70) },
          260,
        );
      }
      if (sidebarBottom.length) {
        timeline.add(sidebarBottom, { opacity: 1, y: 0 }, 470);
      }
      if (topbar.length) timeline.add(topbar, { opacity: 1, y: 0 }, 120);
      return timeline;
    });
  };

  const enterView = () => {
    const views = visible(collect<HTMLElement>(root, ".page-scroll > .view"));
    if (!views.length) return;
    const timeline = createTimeline({
      defaults: {
        duration: duration(560),
        ease: ease(),
      },
    });

    scope.execute(() => {
      utils.set(views, { opacity: 0, y: 18, scale: 0.99 });
      timeline.add(views, { opacity: 1, y: 0, scale: 1 }, 0);

      const groups = [
        ".workspace-head",
        ".workspace-tabs",
        ".workspace-metrics .metric",
        ".overview-grid > .panel",
        ".workspace-toolbar",
        ".section-toolbar",
        ".knowledge-row",
        ".source-card",
        ".source-health",
        ".health",
        ".signal",
        ".timeline-event",
        ".review-summary .metric",
        ".setting-card",
        ".danger-card",
        ".function-topology-panel",
        ".skills-grid .skill-group-card",
        ".empty",
      ];
      let position = 110;
      groups.forEach((selector) => {
        const targets = visible(collect<HTMLElement>(root, selector));
        if (!targets.length) return;
        utils.set(targets, { opacity: 0, y: 15, scale: 0.985 });
        timeline.add(
          targets,
          {
            opacity: 1,
            y: 0,
            scale: 1,
            delay: stagger(isReduced() ? 0 : isCompact() ? 28 : 48),
          },
          position,
        );
        position += Math.min(90, targets.length * 14 + 24);
      });

      const topologyEdges = collect<SVGGeometryElement>(
        root,
        ".function-topology .function-edge",
      );
      if (topologyEdges.length) {
        const drawables = createDrawable(topologyEdges);
        timeline.add(
          drawables,
          {
            draw: "0 1",
            opacity: 1,
            duration: duration(720),
            delay: stagger(isReduced() ? 0 : isCompact() ? 24 : 45),
            ease: ease(),
          },
          120,
        );
      }

      const topologyNodes = visible(
        collect<SVGGElement>(
          root,
          ".function-topology .function-node-group, .function-topology .function-skill-node-group",
        ),
      );
      if (topologyNodes.length) {
        utils.set(topologyNodes, { opacity: 0, y: 12, scale: 0.92 });
        timeline.add(
          topologyNodes,
          {
            opacity: 1,
            y: 0,
            scale: 1,
            delay: stagger(isReduced() ? 0 : isCompact() ? 24 : 42),
            duration: duration(520),
            ease: ease(),
          },
          150,
        );
      }

      const counters = collect<HTMLElement>(root, "[data-motion-counter]");
      counters.forEach((counter) => {
        const value = Number(counter.dataset.motionCounter);
        if (!Number.isFinite(value)) return;
        const state = { value: 0 };
        counter.textContent = "0";
        animate(state, {
          value,
          duration: duration(760),
          ease: ease(),
          delay: 180,
          onUpdate: () => {
            counter.textContent = String(Math.round(state.value));
          },
        });
      });

      return timeline;
    });
  };

  const animateWorkspaceRows = () => {
    enter(collect<HTMLElement>(root, ".workspace-list > *"), {
      y: 10,
      scale: 0.99,
      duration: 420,
      staggerDelay: 42,
    });
  };

  const animatePortal = (open: boolean, contentSelector: string) => {
    const overlay = document.querySelector<HTMLElement>(".dialog-overlay");
    const content = document.querySelector<HTMLElement>(contentSelector);
    if (!overlay || !content) return;

    scope.execute(() => {
      if (!open) {
        return animate([overlay, content], {
          opacity: 0,
          y: 10,
          scale: 0.98,
          duration: duration(220),
          ease: ease(),
        });
      }
      utils.set(overlay, { opacity: 0 });
      utils.set(content, { opacity: 0, y: 18, scale: 0.97 });
      const timeline = createTimeline({
        defaults: { ease: ease() },
      });
      timeline
        .add(overlay, { opacity: 1, duration: duration(260) }, 0)
        .add(
          content,
          {
            opacity: 1,
            y: 0,
            scale: 1,
            duration: duration(520),
          },
          40,
        );
      return timeline;
    });
  };

  return {
    enterShell,
    enterView,
    animateWorkspaceRows,
    animatePortal,
    dispose: () => scope.revert(),
  };
}

export function animatePortalState(
  open: boolean,
  contentSelector: string,
): () => void {
  if (typeof document === "undefined" || !document.body) return () => undefined;
  const motion = createWorkbenchMotion(document.body);
  motion.animatePortal(open, contentSelector);
  return motion.dispose;
}
