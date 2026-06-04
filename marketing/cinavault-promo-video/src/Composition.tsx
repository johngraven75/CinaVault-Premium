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

const C = {
  black: "#030711",
  deep: "#07121f",
  glass: "rgba(8, 20, 34, 0.76)",
  cyan: "#34dbff",
  blue: "#2368ff",
  red: "#ff3b30",
  amber: "#ffb24b",
  green: "#3df2ad",
  white: "#f8fbff",
  muted: "#a8b7c8",
};

const clamp = {
  extrapolateLeft: "clamp" as const,
  extrapolateRight: "clamp" as const,
};

const sceneFrames = (scene: PromoScene) => ({
  from: Math.round(scene.start * 30),
  duration: Math.round((scene.end - scene.start) * 30),
});

const fade = (frame: number, duration: number) =>
  Math.min(
    interpolate(frame, [0, 18], [0, 1], clamp),
    1 - interpolate(frame, [duration - 18, duration], [0, 1], clamp),
  );

const BrandBug = () => (
  <div
    style={{
      position: "absolute",
      top: 52,
      left: 62,
      display: "flex",
      alignItems: "center",
      gap: 18,
      zIndex: 10,
    }}
  >
    <Img
      src={staticFile("assets/cinavault-premium-mark.png")}
      style={{ width: 82, height: 82, filter: "drop-shadow(0 0 22px #ff3b30)" }}
    />
    <div>
      <div style={{ color: C.white, fontSize: 31, fontWeight: 950 }}>CinaVault</div>
      <div style={{ color: C.red, fontSize: 15, fontWeight: 950 }}>
        PREMIUM MEDIA SERVER
      </div>
    </div>
  </div>
);

const CinematicBackdrop = () => {
  const frame = useCurrentFrame();
  const drift = frame * 0.45;
  const sweep = interpolate(Math.sin(frame / 42), [-1, 1], [-240, 1580]);
  return (
    <AbsoluteFill style={{ backgroundColor: C.black, overflow: "hidden" }}>
      <div
        style={{
          position: "absolute",
          inset: 0,
          background:
            "radial-gradient(circle at 18% 24%, rgba(255,59,48,0.42), transparent 28%), radial-gradient(circle at 78% 22%, rgba(52,219,255,0.34), transparent 26%), radial-gradient(circle at 56% 78%, rgba(61,242,173,0.22), transparent 32%), linear-gradient(128deg, #030711 0%, #07121f 38%, #102d38 66%, #240e12 100%)",
        }}
      />
      <div
        style={{
          position: "absolute",
          inset: 0,
          opacity: 0.34,
          backgroundImage:
            "linear-gradient(90deg, rgba(52,219,255,0.22) 1px, transparent 1px), linear-gradient(0deg, rgba(255,255,255,0.10) 1px, transparent 1px)",
          backgroundSize: "64px 64px",
          transform: `translateY(${-drift}px)`,
        }}
      />
      {Array.from({ length: 46 }).map((_, i) => {
        const x = (i * 97 + frame * (0.7 + (i % 5) * 0.1)) % 1920;
        const y = (i * 53 + frame * (0.24 + (i % 3) * 0.08)) % 1080;
        const size = 3 + (i % 5);
        return (
          <div
            key={i}
            style={{
              position: "absolute",
              left: x,
              top: y,
              width: size,
              height: size,
              backgroundColor: i % 3 === 0 ? C.amber : i % 3 === 1 ? C.cyan : C.green,
              boxShadow: "0 0 18px currentColor",
              opacity: 0.42,
            }}
          />
        );
      })}
      <div
        style={{
          position: "absolute",
          top: -260,
          left: sweep,
          width: 360,
          height: 1600,
          transform: "rotate(21deg)",
          background:
            "linear-gradient(90deg, transparent, rgba(255,255,255,0.24), rgba(52,219,255,0.28), transparent)",
          opacity: 0.52,
        }}
      />
      <div
        style={{
          position: "absolute",
          inset: 0,
          background:
            "linear-gradient(180deg, rgba(3,7,17,0.22), transparent 42%, rgba(3,7,17,0.88))",
        }}
      />
    </AbsoluteFill>
  );
};

const SceneShell: React.FC<{ scene: PromoScene; children: React.ReactNode }> = ({
  scene,
  children,
}) => {
  const frame = useCurrentFrame();
  const duration = sceneFrames(scene).duration;
  return (
    <AbsoluteFill style={{ opacity: fade(frame, duration), backgroundColor: C.black }}>
      <CinematicBackdrop />
      <BrandBug />
      {children}
      <KnowledgePanel scene={scene} />
      <Caption scene={scene} />
      <TitleBlock scene={scene} />
    </AbsoluteFill>
  );
};

const TitleBlock = ({ scene }: { scene: PromoScene }) => {
  const frame = useCurrentFrame();
  const enter = spring({
    frame,
    fps: 30,
    config: { damping: 22, stiffness: 135 },
  });
  return (
    <div
      style={{
        position: "absolute",
        left: 76,
        bottom: 150,
        width: 1020,
        transform: `translateY(${(1 - enter) * 42}px)`,
      }}
    >
      <div
        style={{
          color: C.amber,
          fontSize: 24,
          fontWeight: 950,
          marginBottom: 16,
          letterSpacing: 0,
        }}
      >
        {scene.kicker}
      </div>
      <div
        style={{
          color: C.white,
          fontSize: 72,
          fontWeight: 1000,
          lineHeight: 1,
          textShadow: "0 0 24px rgba(52,219,255,0.26)",
        }}
      >
        {scene.title}
      </div>
      <div
        style={{
          color: C.muted,
          fontSize: 30,
          fontWeight: 700,
          lineHeight: 1.25,
          marginTop: 18,
          maxWidth: 900,
        }}
      >
        {scene.subtitle}
      </div>
    </div>
  );
};

const KnowledgePanel = ({ scene }: { scene: PromoScene }) => {
  const frame = useCurrentFrame();
  return (
    <div
      style={{
        position: "absolute",
        right: 62,
        top: 58,
        width: 470,
        display: "grid",
        gap: 14,
        zIndex: 20,
      }}
    >
      {scene.facts.map((fact, i) => {
        const enter = spring({
          frame: frame - i * 8,
          fps: 30,
          config: { damping: 17, stiffness: 150 },
        });
        return (
          <div
            key={fact}
            style={{
              minHeight: 66,
              display: "flex",
              alignItems: "center",
              gap: 16,
              padding: "0 20px",
              transform: `translateX(${(1 - enter) * 90}px)`,
              opacity: enter,
              color: C.white,
              fontSize: 22,
              fontWeight: 900,
              backgroundColor: C.glass,
              border: `1px solid ${i === 0 ? C.red : i === 1 ? C.cyan : C.amber}`,
              boxShadow: `0 0 28px ${i === 0 ? "rgba(255,59,48,0.28)" : i === 1 ? "rgba(52,219,255,0.25)" : "rgba(255,178,75,0.24)"}`,
            }}
          >
            <div
              style={{
                width: 16,
                height: 16,
                backgroundColor: i === 0 ? C.red : i === 1 ? C.cyan : C.amber,
                boxShadow: "0 0 18px currentColor",
              }}
            />
            {fact}
          </div>
        );
      })}
    </div>
  );
};

const Caption = ({ scene }: { scene: PromoScene }) => (
  <div
    style={{
      position: "absolute",
      left: 76,
      right: 76,
      bottom: 52,
      minHeight: 58,
      padding: "12px 22px",
      color: "#dceeff",
      fontSize: 25,
      fontWeight: 750,
      lineHeight: 1.22,
      backgroundColor: "rgba(3,7,17,0.72)",
      border: "1px solid rgba(52,219,255,0.32)",
      zIndex: 30,
    }}
  >
    {scene.voiceover}
  </div>
);

const HoloCore = ({ compact = false }: { compact?: boolean }) => {
  const frame = useCurrentFrame();
  const spin = frame * 0.7;
  const pulse = Math.sin(frame / 10) * 0.5 + 0.5;
  return (
    <div
      style={{
        position: "absolute",
        left: compact ? 770 : 700,
        top: compact ? 305 : 235,
        width: compact ? 380 : 520,
        height: compact ? 380 : 520,
      }}
    >
      {[0, 1, 2].map((ring) => (
        <div
          key={ring}
          style={{
            position: "absolute",
            inset: 34 + ring * 46,
            borderRadius: 999,
            border: `3px solid ${ring === 0 ? C.cyan : ring === 1 ? C.red : C.amber}`,
            opacity: 0.42 + pulse * 0.18,
            transform: `rotate(${spin * (ring % 2 === 0 ? 1 : -1)}deg)`,
            boxShadow: "0 0 34px currentColor",
          }}
        />
      ))}
      <div
        style={{
          position: "absolute",
          inset: compact ? 120 : 170,
          display: "grid",
          placeItems: "center",
          backgroundColor: "rgba(3,7,17,0.88)",
          border: "2px solid rgba(255,255,255,0.26)",
          boxShadow: "0 0 90px rgba(52,219,255,0.42)",
        }}
      >
        <Img
          src={staticFile("assets/cinavault-premium-mark.png")}
          style={{ width: compact ? 118 : 150 }}
        />
      </div>
    </div>
  );
};

const OpeningScene = () => {
  const frame = useCurrentFrame();
  const count = Math.round(interpolate(frame, [0, 170], [880, 11064], clamp));
  return (
    <SceneShell scene={scenes[0]}>
      <HoloCore />
      <div
        style={{
          position: "absolute",
          right: 650,
          top: 210,
          color: C.white,
          fontSize: 96,
          fontWeight: 1000,
          textAlign: "right",
          textShadow: "0 0 30px rgba(52,219,255,0.42)",
        }}
      >
        {count.toLocaleString()}
        <div style={{ color: C.muted, fontSize: 28, fontWeight: 850 }}>
          titles under command
        </div>
      </div>
      <TileStorm />
    </SceneShell>
  );
};

const TileStorm = () => {
  const frame = useCurrentFrame();
  const labels = ["MOVIES", "SERIES", "LIVE TV", "POSTERS", "NAS", "CLOUD", "REMOTE", "AI"];
  return (
    <>
      {labels.map((label, i) => {
        const x = interpolate(frame, [0, 160], [-260 + i * 80, 1170 + i * 24], clamp);
        const y = 700 - (i % 4) * 96 + Math.sin((frame + i * 18) / 17) * 18;
        return (
          <div
            key={label}
            style={{
              position: "absolute",
              left: x,
              top: y,
              width: 220,
              height: 74,
              display: "grid",
              placeItems: "center",
              color: C.white,
              fontSize: 24,
              fontWeight: 950,
              backgroundColor: "rgba(8,20,34,0.78)",
              border: `1px solid ${i % 2 === 0 ? C.cyan : C.red}`,
              boxShadow: "0 0 28px rgba(52,219,255,0.18)",
            }}
          >
            {label}
          </div>
        );
      })}
    </>
  );
};

const CommandCenterScene = () => {
  const nodes = ["Local", "NAS", "Cloud", "Live TV", "Remote", "Security", "Plugins"];
  const frame = useCurrentFrame();
  return (
    <SceneShell scene={scenes[1]}>
      <HoloCore compact />
      {nodes.map((node, i) => {
        const angle = (Math.PI * 2 * i) / nodes.length + frame / 90;
        const x = 960 + Math.cos(angle) * 520;
        const y = 500 + Math.sin(angle) * 260;
        return (
          <div key={node}>
            <div
              style={{
                position: "absolute",
                left: 960,
                top: 500,
                width: Math.hypot(x - 960, y - 500),
                height: 3,
                backgroundColor: "rgba(52,219,255,0.34)",
                transformOrigin: "0 50%",
                transform: `rotate(${Math.atan2(y - 500, x - 960)}rad)`,
              }}
            />
            <div
              style={{
                position: "absolute",
                left: x - 86,
                top: y - 42,
                width: 172,
                height: 84,
                display: "grid",
                placeItems: "center",
                color: C.white,
                fontSize: 25,
                fontWeight: 950,
                backgroundColor: i % 3 === 0 ? "rgba(255,59,48,0.76)" : i % 3 === 1 ? "rgba(52,219,255,0.70)" : "rgba(61,242,173,0.70)",
                border: "1px solid rgba(255,255,255,0.28)",
                boxShadow: "0 0 30px rgba(255,255,255,0.13)",
              }}
            >
              {node}
            </div>
          </div>
        );
      })}
    </SceneShell>
  );
};

const IntelligenceScene = () => {
  const frame = useCurrentFrame();
  const rows = [
    ["AI metadata gather", "Provider lookup + local fallback"],
    ["Poster recovery", "Forward-facing artwork populated"],
    ["Embedded titles", "Filename noise cleaned"],
    ["Diagnostics", "Network, source, provider visibility"],
  ];
  return (
    <SceneShell scene={scenes[2]}>
      <div style={{ position: "absolute", left: 130, top: 205, width: 1160 }}>
        {rows.map(([name, detail], i) => {
          const fill = interpolate(frame - i * 20, [0, 82], [0, 1], clamp);
          return (
            <div
              key={name}
              style={{
                height: 112,
                marginBottom: 22,
                display: "grid",
                gridTemplateColumns: "340px 1fr 110px",
                alignItems: "center",
                padding: "0 28px",
                backgroundColor: C.glass,
                border: "1px solid rgba(52,219,255,0.34)",
                boxShadow: "0 0 34px rgba(52,219,255,0.12)",
              }}
            >
              <div style={{ color: C.white, fontSize: 29, fontWeight: 950 }}>{name}</div>
              <div>
                <div style={{ color: C.muted, fontSize: 21, fontWeight: 750, marginBottom: 14 }}>
                  {detail}
                </div>
                <div style={{ height: 14, backgroundColor: "rgba(255,255,255,0.12)" }}>
                  <div
                    style={{
                      width: `${fill * 100}%`,
                      height: "100%",
                      background:
                        i % 2 === 0
                          ? `linear-gradient(90deg, ${C.cyan}, ${C.green})`
                          : `linear-gradient(90deg, ${C.red}, ${C.amber})`,
                      boxShadow: "0 0 22px currentColor",
                    }}
                  />
                </div>
              </div>
              <div
                style={{
                  color: fill > 0.96 ? C.green : C.amber,
                  fontSize: 25,
                  fontWeight: 950,
                  textAlign: "right",
                }}
              >
                {fill > 0.96 ? "LOCKED" : `${Math.round(fill * 100)}%`}
              </div>
            </div>
          );
        })}
      </div>
    </SceneShell>
  );
};

const ScaleScene = () => {
  const frame = useCurrentFrame();
  return (
    <SceneShell scene={scenes[3]}>
      <div
        style={{
          position: "absolute",
          left: 170,
          top: 210,
          width: 1120,
          height: 468,
          padding: 44,
          backgroundColor: C.glass,
          border: "1px solid rgba(52,219,255,0.42)",
          boxShadow: "0 0 65px rgba(52,219,255,0.18)",
        }}
      >
        <SystemBar label="Legacy full-library reload" value={interpolate(frame, [0, 210], [72, 42], clamp)} color="#8b95a2" />
        <SystemBar label="CinaVault paged responsive flow" value={interpolate(frame, [0, 210], [46, 96], clamp)} color={C.green} />
        <SystemBar label="Poster cache stability" value={interpolate(frame, [0, 210], [34, 91], clamp)} color={C.amber} />
        <div
          style={{
            marginTop: 34,
            display: "grid",
            gridTemplateColumns: "repeat(3, 1fr)",
            gap: 18,
          }}
        >
          {["Source health", "Task progress", "Safer cleanup"].map((text) => (
            <div
              key={text}
              style={{
                height: 82,
                display: "grid",
                placeItems: "center",
                color: C.white,
                fontSize: 25,
                fontWeight: 950,
                backgroundColor: "rgba(255,255,255,0.08)",
                border: "1px solid rgba(255,255,255,0.22)",
              }}
            >
              {text}
            </div>
          ))}
        </div>
      </div>
    </SceneShell>
  );
};

const SystemBar = ({ label, value, color }: { label: string; value: number; color: string }) => (
  <div style={{ marginBottom: 28 }}>
    <div
      style={{
        display: "flex",
        justifyContent: "space-between",
        color: C.white,
        fontSize: 26,
        fontWeight: 900,
        marginBottom: 12,
      }}
    >
      <span>{label}</span>
      <span>{Math.round(value)}%</span>
    </div>
    <div style={{ height: 22, backgroundColor: "rgba(255,255,255,0.12)" }}>
      <div style={{ height: "100%", width: `${value}%`, backgroundColor: color }} />
    </div>
  </div>
);

const RealUiScene = () => {
  const frame = useCurrentFrame();
  const zoom = interpolate(frame, [0, 240], [1.04, 1.16], clamp);
  return (
    <SceneShell scene={scenes[4]}>
      <div
        style={{
          position: "absolute",
          left: 74,
          top: 142,
          width: 1380,
          height: 724,
          overflow: "hidden",
          border: "2px solid rgba(52,219,255,0.62)",
          boxShadow: "0 0 80px rgba(52,219,255,0.26)",
        }}
      >
        <Img
          src={staticFile("assets/cinavault-ui-build-127.png")}
          style={{ width: "100%", height: "100%", objectFit: "cover", transform: `scale(${zoom})` }}
        />
      </div>
      <div
        style={{
          position: "absolute",
          left: 120,
          top: 186,
          color: C.white,
          fontSize: 29,
          fontWeight: 950,
          padding: "14px 22px",
          backgroundColor: "rgba(3,7,17,0.78)",
          border: `1px solid ${C.red}`,
          boxShadow: "0 0 26px rgba(255,59,48,0.35)",
        }}
      >
        REAL CINA VAULT INTERFACE
      </div>
    </SceneShell>
  );
};

const FinalScene = () => {
  const frame = useCurrentFrame();
  const pop = spring({ frame, fps: 30, config: { damping: 18, stiffness: 120 } });
  return (
    <SceneShell scene={scenes[5]}>
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
            style={{ width: 790, marginBottom: 46, filter: "drop-shadow(0 0 42px rgba(52,219,255,0.45))" }}
          />
          <div style={{ color: C.white, fontSize: 82, fontWeight: 1000 }}>
            Upgrade to CinaVault Premium
          </div>
          <div style={{ color: C.amber, fontSize: 34, fontWeight: 950, marginTop: 24 }}>
            High-tech control for the library you built.
          </div>
        </div>
      </div>
    </SceneShell>
  );
};

export const CinaVaultPromo = () => {
  const { fps: videoFps } = useVideoConfig();
  if (videoFps !== 30) {
    throw new Error("CinaVaultPromo is timed for 30 fps.");
  }

  return (
    <AbsoluteFill style={{ backgroundColor: C.black }}>
      <Audio src={staticFile("audio/cinavault-energy-bed-v2.wav")} volume={0.16} />
      <Audio src={staticFile("audio/cinavault-promo-voiceover-v2.mp3")} volume={1} />
      <Sequence from={sceneFrames(scenes[0]).from} durationInFrames={sceneFrames(scenes[0]).duration}>
        <OpeningScene />
      </Sequence>
      <Sequence from={sceneFrames(scenes[1]).from} durationInFrames={sceneFrames(scenes[1]).duration}>
        <CommandCenterScene />
      </Sequence>
      <Sequence from={sceneFrames(scenes[2]).from} durationInFrames={sceneFrames(scenes[2]).duration}>
        <IntelligenceScene />
      </Sequence>
      <Sequence from={sceneFrames(scenes[3]).from} durationInFrames={sceneFrames(scenes[3]).duration}>
        <ScaleScene />
      </Sequence>
      <Sequence from={sceneFrames(scenes[4]).from} durationInFrames={sceneFrames(scenes[4]).duration}>
        <RealUiScene />
      </Sequence>
      <Sequence from={sceneFrames(scenes[5]).from} durationInFrames={sceneFrames(scenes[5]).duration}>
        <FinalScene />
      </Sequence>
    </AbsoluteFill>
  );
};
