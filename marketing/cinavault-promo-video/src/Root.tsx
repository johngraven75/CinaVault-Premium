import "./index.css";
import { Composition } from "remotion";
import { CinaVaultPromo } from "./Composition";
import { durationInFrames, fps } from "./timeline";

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="CinaVaultPromo"
        component={CinaVaultPromo}
        durationInFrames={durationInFrames}
        fps={fps}
        width={1920}
        height={1080}
      />
    </>
  );
};
