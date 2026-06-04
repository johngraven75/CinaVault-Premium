import {
  AbsoluteFill,
  Audio,
  Img,
  Sequence,
  interpolate,
  spring,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import { scenes, type PromoScene } from "./timeline";

const colors = {
  ink: "#07111f",
  panel: "#0d2030",
  cyan: "#38d4ff",
  aqua: "#1cae9d",
  red: "#ff4936",
  amber: "#f4a84f",
  text: "#f6fbff",
  muted: "#aebbc8",
};

const ease = (frame: number, start: number, end: number) =>
  interpolate(frame, [start, end], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

const sceneFrames = (scene: PromoScene) => ({
  from: Math.round(scene.start * 30),
  duration: Math.round((scene.end - scene.start) * 30),
});

const Shell: React.FC<{ children: React.ReactNode; scene: PromoScene }> = ({
  children,
  scene,
}) => {
  const frame = useCurrentFrame();
  const duration = sceneFrames(scene).duration;
  const fadeIn = ease(frame, 0, 14);
  const fadeOut = 1 - ease(frame, duration - 14, duration);
  const opacity = Math.min(fadeIn, fadeOut);

  return (
    <AbsoluteFill style={{ opacity, backgroundColor: colors.ink }}>
      <Backdrop />
      <Spotlight />
      {children}
      <KnowledgeRail scene={scene} />
      <LowerThird scene={scene} />
    </AbsoluteFill>
  );
};

const Backdrop: React.FC = () => {
  const frame = useCurrentFrame();
  const drift = frame * 0.18;
  return (
    <AbsoluteFill>
      <div
        style={{
          position: "absolute",
          inset: 0,
          background:
            "linear-gradient(118deg, #07111f 0%, #102235 36%, #113b40 62%, #32160e 100%)",
        }}
      />
      <div
        style={{
          position: "absolute",
          inset: 0,
          opacity: 0.35,
          backgroundImage:
            "linear-gradient(90deg, rgba(56,212,255,0.16) 1px, transparent 1px), linear-gradient(0deg, rgba(255,255,255,0.10) 1px, transparent 1px)",
          backgroundSize: "72px 72px",
          transform: `translateY(${-drift}px)`,
        }}
      />
      <div
        style={{
          position: "absolute",
          left: 0,
          right: 0,
          bottom: 0,
          height: 170,
          background:
            "linear-gradient(0deg, rgba(7,17,31,0.96), rgba(7,17,31,0))",
        }}
      />
      <div
        style={{
          position: "absolute",
          inset: 0,
          opacity: 0.08,
          backgroundImage:
            "repeating-linear-gradient(0deg, #ffffff 0px, #ffffff 1px, transparent 1px, transparent 7px)",
        }}
      />
    </AbsoluteFill>
  );
};

const Spotlight: React.FC = () => {
  const frame = useCurrentFrame();
  const x = interpolate(Math.sin(frame / 28), [-1, 1], [8, 72]);
  return (
    <div
      style={{
        position: "absolute",
        left: `${x}%`,
        top: "-18%",
        width: 560,
        height: 1420,
        opacity: 0.28,
        transform: `rotate(${18 + Math.sin(frame / 40) * 5}deg)`,
        background:
          "linear-gradient(90deg, rgba(56,212,255,0), rgba(56,212,255,0.32), rgba(244,168,79,0))",
      }}
    />
  );
};

const LowerThird: React.FC<{ scene: PromoScene }> = ({ scene }) => {
  const frame = useCurrentFrame();
  const enter = spring({
    frame,
    fps: 30,
    config: { damping: 24, stiffness: 140 },
  });
  return (
    <div
      style={{
        position: "absolute",
        left: 82,
        bottom: 74,
        width: 940,
        transform: `translateY(${(1 - enter) * 28}px)`,
      }}
    >
      <div
        style={{
          color: colors.amber,
          fontSize: 24,
          fontWeight: 800,
          marginBottom: 14,
        }}
      >
        {scene.id.replace("-", " ").toUpperCase()}
      </div>
      <div
        style={{
          color: colors.text,
          fontSize: 58,
          fontWeight: 900,
          lineHeight: 1.02,
          maxWidth: 980,
        }}
      >
        {scene.title}
      </div>
      <div
        style={{
          color: colors.muted,
          fontSize: 27,
          fontWeight: 600,
          marginTop: 16,
          maxWidth: 900,
          lineHeight: 1.28,
        }}
      >
        {scene.subtitle}
      </div>
    </div>
  );
};

const KnowledgeRail: React.FC<{ scene: PromoScene }> = ({ scene }) => {
  const frame = useCurrentFrame();
  return (
    <div
      style={{
        position: "absolute",
        right: 74,
        top: 62,
        width: 420,
        display: "grid",
        gap: 14,
      }}
    >
      {scene.facts.map((fact, index) => {
        const enter = spring({
          frame: frame - index * 8,
          fps: 30,
          config: { damping: 18, stiffness: 150 },
        });
        const shimmer = Math.sin((frame + index * 23) / 14) * 0.5 + 0.5;
        return (
          <div
            key={fact}
            style={{
              transform: `translateX(${(1 - enter) * 80}px)`,
              opacity: enter,
              minHeight: 58,
              display: "flex",
              alignItems: "center",
              gap: 16,
              padding: "0 18px",
              color: colors.text,
              fontSize: 21,
              fontWeight: 850,
              backgroundColor: "rgba(13,32,48,0.78)",
              border: "1px solid rgba(56,212,255,0.46)",
              boxShadow: `0 0 ${12 + shimmer * 26}px rgba(56,212,255,0.20)`,
            }}
          >
            <span
              style={{
                width: 13,
                height: 13,
                backgroundColor:
                  index === 0 ? colors.red : index === 1 ? colors.cyan : colors.amber,
                boxShadow: "0 0 16px currentColor",
              }}
            />
            {fact}
          </div>
        );
      })}
    </div>
  );
};

const HeaderBrand: React.FC = () => (
  <div
    style={{
      position: "absolute",
      left: 70,
      top: 54,
      display: "flex",
      alignItems: "center",
      gap: 18,
    }}
  >
    <Img
      src={staticFile("assets/cinavault-premium-mark.png")}
      style={{ width: 72, height: 72, objectFit: "contain" }}
    />
    <div>
      <div style={{ color: colors.text, fontSize: 28, fontWeight: 900 }}>
        CinaVault
      </div>
      <div style={{ color: colors.red, fontSize: 15, fontWeight: 900 }}>
        PREMIUM MEDIA SERVER
      </div>
    </div>
  </div>
);

const MediaTiles: React.FC = () => {
  const frame = useCurrentFrame();
  const titles = [
    "Movies",
    "Live TV",
    "Series",
    "NAS",
    "Cloud",
    "Posters",
    "Metadata",
    "Remote",
    "Duplicates",
    "Security",
  ];
  return (
    <Shell scene={scenes[0]}>
      <HeaderBrand />
      <div
        style={{
          position: "absolute",
          top: 188,
          left: 92,
          right: 92,
          display: "grid",
          gridTemplateColumns: "repeat(5, 1fr)",
          gap: 18,
        }}
      >
        {titles.map((title, index) => {
          const local = frame - index * 4;
          const y = interpolate(local, [0, 50], [120, 0], {
            extrapolateLeft: "clamp",
            extrapolateRight: "clamp",
          });
          const glow = Math.sin((frame + index * 11) / 16) * 0.5 + 0.5;
          return (
            <div
              key={title}
              style={{
                height: 118,
                border: "1px solid rgba(56,212,255,0.45)",
                backgroundColor: "rgba(13,32,48,0.82)",
                transform: `translateY(${y}px)`,
                boxShadow: `0 0 ${18 + glow * 22}px rgba(56,212,255,0.22)`,
                padding: 18,
                display: "flex",
                flexDirection: "column",
                justifyContent: "space-between",
              }}
            >
              <div style={{ color: colors.text, fontSize: 23, fontWeight: 800 }}>
                {title}
              </div>
              <div
                style={{
                  height: 7,
                  width: `${42 + ((frame + index * 17) % 54)}%`,
                  backgroundColor: index % 3 === 0 ? colors.red : colors.cyan,
                }}
              />
            </div>
          );
        })}
      </div>
      <Counter />
    </Shell>
  );
};

const Counter: React.FC = () => {
  const frame = useCurrentFrame();
  const count = Math.round(interpolate(frame, [0, 130], [240, 11064], {
    extrapolateRight: "clamp",
  }));
  return (
    <div
      style={{
        position: "absolute",
        right: 96,
        bottom: 242,
        color: colors.text,
        fontSize: 82,
        fontWeight: 950,
      }}
    >
      {count.toLocaleString()}
      <div style={{ color: colors.muted, fontSize: 25, fontWeight: 700 }}>
        items indexed
      </div>
    </div>
  );
};

const NetworkReveal: React.FC = () => {
  const frame = useCurrentFrame();
  const pulse = Math.sin(frame / 12) * 0.5 + 0.5;
  const nodes = [
    ["Library", 720, 250],
    ["NAS", 950, 340],
    ["Cloud", 845, 520],
    ["Remote", 1110, 500],
    ["Live TV", 1130, 210],
  ] as const;
  return (
    <Shell scene={scenes[1]}>
      <HeaderBrand />
      <Img
        src={staticFile("assets/cinavault-premium-brand-full.png")}
        style={{
          position: "absolute",
          left: 92,
          top: 244,
          width: 560,
          objectFit: "contain",
          filter: "drop-shadow(0 30px 56px rgba(0,0,0,0.45))",
        }}
      />
      <div
        style={{
          position: "absolute",
          left: 690,
          top: 170,
          width: 560,
          height: 480,
        }}
      >
        {nodes.map(([label, x, y], index) => (
          <div key={label}>
            <div
              style={{
                position: "absolute",
                left: 280,
                top: 240,
                width: Math.hypot(x - 970, y - 410),
                height: 2,
                backgroundColor: "rgba(56,212,255,0.35)",
                transformOrigin: "0 50%",
                transform: `rotate(${Math.atan2(y - 410, x - 970)}rad)`,
              }}
            />
            <div
              style={{
                position: "absolute",
                left: x - 690,
                top: y - 170,
                transform: `scale(${0.94 + pulse * 0.06})`,
                width: 132,
                height: 58,
                display: "grid",
                placeItems: "center",
                color: colors.text,
                fontSize: 22,
                fontWeight: 850,
                backgroundColor:
                  index % 2 === 0 ? "rgba(28,174,157,0.76)" : "rgba(244,168,79,0.78)",
                border: "1px solid rgba(255,255,255,0.28)",
              }}
            >
              {label}
            </div>
          </div>
        ))}
        <div
          style={{
            position: "absolute",
            left: 210,
            top: 168,
            width: 142,
            height: 142,
            borderRadius: 999,
            display: "grid",
            placeItems: "center",
            backgroundColor: "rgba(13,32,48,0.92)",
            border: "2px solid rgba(56,212,255,0.9)",
            boxShadow: `0 0 ${40 + pulse * 30}px rgba(56,212,255,0.42)`,
          }}
        >
          <Img
            src={staticFile("assets/cinavault-premium-mark.png")}
            style={{ width: 86, height: 86 }}
          />
        </div>
      </div>
    </Shell>
  );
};

const FeatureAutomation: React.FC = () => {
  const frame = useCurrentFrame();
  const features = [
    ["AI metadata", "Provider lookup and embedded-title fallback"],
    ["Poster discovery", "Artwork appears on the forward UI"],
    ["Duplicate cleanup", "Child rows removed before media rows"],
    ["Diagnostics", "Network, sources, providers, enrichment"],
  ];
  return (
    <Shell scene={scenes[2]}>
      <HeaderBrand />
      <div style={{ position: "absolute", left: 112, top: 176, right: 112 }}>
        {features.map(([name, detail], index) => {
          const local = frame - index * 18;
          const fill = interpolate(local, [0, 52], [0, 1], {
            extrapolateLeft: "clamp",
            extrapolateRight: "clamp",
          });
          return (
            <div
              key={name}
              style={{
                height: 104,
                marginBottom: 22,
                backgroundColor: "rgba(13,32,48,0.84)",
                border: "1px solid rgba(255,255,255,0.18)",
                display: "grid",
                gridTemplateColumns: "300px 1fr 104px",
                alignItems: "center",
                padding: "0 28px",
              }}
            >
              <div style={{ color: colors.text, fontSize: 30, fontWeight: 900 }}>
                {name}
              </div>
              <div>
                <div style={{ color: colors.muted, fontSize: 21, marginBottom: 14 }}>
                  {detail}
                </div>
                <div
                  style={{
                    height: 10,
                    backgroundColor: "rgba(255,255,255,0.12)",
                    overflow: "hidden",
                  }}
                >
                  <div
                    style={{
                      width: `${fill * 100}%`,
                      height: "100%",
                      backgroundColor: index % 2 === 0 ? colors.cyan : colors.amber,
                    }}
                  />
                </div>
              </div>
              <div
                style={{
                  color: fill > 0.98 ? colors.aqua : colors.muted,
                  fontSize: 28,
                  fontWeight: 900,
                  textAlign: "right",
                }}
              >
                {fill > 0.98 ? "DONE" : `${Math.round(fill * 100)}%`}
              </div>
            </div>
          );
        })}
      </div>
    </Shell>
  );
};

const AccessLayer: React.FC = () => {
  const frame = useCurrentFrame();
  const items = ["Local", "NAS", "Cloud", "Live TV", "Remote", "Security"];
  return (
    <Shell scene={scenes[3]}>
      <HeaderBrand />
      <div
        style={{
          position: "absolute",
          left: 190,
          top: 160,
          right: 190,
          height: 500,
        }}
      >
        <div
          style={{
            position: "absolute",
            left: "50%",
            top: "50%",
            transform: "translate(-50%, -50%)",
            width: 220,
            height: 220,
            display: "grid",
            placeItems: "center",
            backgroundColor: "rgba(13,32,48,0.95)",
            border: "2px solid rgba(56,212,255,0.8)",
            boxShadow: "0 0 70px rgba(56,212,255,0.26)",
          }}
        >
          <Img
            src={staticFile("assets/cinavault-premium-mark.png")}
            style={{ width: 128 }}
          />
        </div>
        {items.map((item, index) => {
          const angle = (Math.PI * 2 * index) / items.length + frame / 220;
          const x = Math.cos(angle) * 390;
          const y = Math.sin(angle) * 190;
          return (
            <div
              key={item}
              style={{
                position: "absolute",
                left: `calc(50% + ${x}px - 82px)`,
                top: `calc(50% + ${y}px - 42px)`,
                width: 164,
                height: 84,
                display: "grid",
                placeItems: "center",
                color: colors.text,
                fontSize: 26,
                fontWeight: 900,
                backgroundColor:
                  index % 2 === 0 ? "rgba(28,174,157,0.74)" : "rgba(255,73,54,0.7)",
                border: "1px solid rgba(255,255,255,0.25)",
              }}
            >
              {item}
            </div>
          );
        })}
      </div>
    </Shell>
  );
};

const Scalability: React.FC = () => {
  const frame = useCurrentFrame();
  const local = frame;
  const legacy = interpolate(local, [0, 95], [22, 62], {
    extrapolateRight: "clamp",
  });
  const cv = interpolate(local, [0, 95], [38, 95], {
    extrapolateRight: "clamp",
  });
  return (
    <Shell scene={scenes[4]}>
      <HeaderBrand />
      <div
        style={{
          position: "absolute",
          left: 170,
          top: 170,
          right: 170,
          height: 430,
          backgroundColor: "rgba(13,32,48,0.8)",
          border: "1px solid rgba(56,212,255,0.35)",
          padding: 54,
        }}
      >
        <MetricBar label="Legacy workflow" value={legacy} color="#7f8b98" />
        <MetricBar label="CinaVault responsive path" value={cv} color={colors.aqua} />
        <div
          style={{
            marginTop: 52,
            display: "grid",
            gridTemplateColumns: "repeat(3, 1fr)",
            gap: 24,
          }}
        >
          {["Paged loading", "Poster cache", "Provider checks"].map((name) => (
            <div
              key={name}
              style={{
                height: 104,
                color: colors.text,
                fontSize: 26,
                fontWeight: 900,
                display: "grid",
                placeItems: "center",
                border: "1px solid rgba(255,255,255,0.22)",
                backgroundColor: "rgba(244,168,79,0.2)",
              }}
            >
              {name}
            </div>
          ))}
        </div>
      </div>
    </Shell>
  );
};

const MetricBar: React.FC<{ label: string; value: number; color: string }> = ({
  label,
  value,
  color,
}) => (
  <div style={{ marginBottom: 36 }}>
    <div
      style={{
        display: "flex",
        justifyContent: "space-between",
        color: colors.text,
        fontSize: 28,
        fontWeight: 900,
        marginBottom: 14,
      }}
    >
      <span>{label}</span>
      <span>{Math.round(value)}%</span>
    </div>
    <div style={{ height: 28, backgroundColor: "rgba(255,255,255,0.12)" }}>
      <div style={{ height: "100%", width: `${value}%`, backgroundColor: color }} />
    </div>
  </div>
);

const InterfaceProof: React.FC = () => {
  const frame = useCurrentFrame();
  const local = frame;
  const zoom = interpolate(local, [0, 150], [1.06, 1.16], {
    extrapolateRight: "clamp",
  });
  return (
    <Shell scene={scenes[5]}>
      <div
        style={{
          position: "absolute",
          inset: 42,
          overflow: "hidden",
          border: "1px solid rgba(56,212,255,0.46)",
          boxShadow: "0 34px 90px rgba(0,0,0,0.54)",
        }}
      >
        <Img
          src={staticFile("assets/cinavault-ui-build-127.png")}
          style={{
            width: "100%",
            height: "100%",
            objectFit: "cover",
            transform: `scale(${zoom})`,
          }}
        />
      </div>
      <div
        style={{
          position: "absolute",
          left: 74,
          top: 70,
          padding: "14px 24px",
          color: colors.text,
          fontSize: 24,
          fontWeight: 900,
          backgroundColor: "rgba(7,17,31,0.72)",
          border: "1px solid rgba(255,255,255,0.2)",
        }}
      >
        Real product interface
      </div>
    </Shell>
  );
};

const Cta: React.FC = () => {
  const frame = useCurrentFrame();
  const pop = spring({
    frame,
    fps: 30,
    config: { damping: 18, stiffness: 120 },
  });
  return (
    <Shell scene={scenes[6]}>
      <div
        style={{
          position: "absolute",
          inset: 0,
          display: "grid",
          placeItems: "center",
          textAlign: "center",
        }}
      >
        <div style={{ transform: `scale(${0.82 + pop * 0.18})` }}>
          <Img
            src={staticFile("assets/cinavault-premium-brand-full.png")}
            style={{ width: 720, objectFit: "contain", marginBottom: 42 }}
          />
          <div style={{ color: colors.text, fontSize: 68, fontWeight: 950 }}>
            Upgrade to CinaVault today
          </div>
          <div
            style={{
              color: colors.amber,
              fontSize: 32,
              fontWeight: 900,
              marginTop: 22,
            }}
          >
            Learn more at your CinaVault site
          </div>
        </div>
      </div>
    </Shell>
  );
};

export const CinaVaultPromo = () => {
  const { fps: videoFps } = useVideoConfig();
  if (videoFps !== 30) {
    throw new Error("CinaVaultPromo is timed for 30 fps.");
  }

  return (
    <AbsoluteFill style={{ backgroundColor: colors.ink }}>
      <Audio src={staticFile("audio/cinavault-energy-bed.wav")} volume={0.12} />
      <Audio src={staticFile("audio/cinavault-promo-voiceover.wav")} />
      <Sequence from={sceneFrames(scenes[0]).from} durationInFrames={sceneFrames(scenes[0]).duration}>
        <MediaTiles />
      </Sequence>
      <Sequence from={sceneFrames(scenes[1]).from} durationInFrames={sceneFrames(scenes[1]).duration}>
        <NetworkReveal />
      </Sequence>
      <Sequence from={sceneFrames(scenes[2]).from} durationInFrames={sceneFrames(scenes[2]).duration}>
        <FeatureAutomation />
      </Sequence>
      <Sequence from={sceneFrames(scenes[3]).from} durationInFrames={sceneFrames(scenes[3]).duration}>
        <AccessLayer />
      </Sequence>
      <Sequence from={sceneFrames(scenes[4]).from} durationInFrames={sceneFrames(scenes[4]).duration}>
        <Scalability />
      </Sequence>
      <Sequence from={sceneFrames(scenes[5]).from} durationInFrames={sceneFrames(scenes[5]).duration}>
        <InterfaceProof />
      </Sequence>
      <Sequence from={sceneFrames(scenes[6]).from} durationInFrames={sceneFrames(scenes[6]).duration}>
        <Cta />
      </Sequence>
    </AbsoluteFill>
  );
};
