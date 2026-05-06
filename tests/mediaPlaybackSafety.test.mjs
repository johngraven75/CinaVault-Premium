import test from "node:test";
import assert from "node:assert/strict";

import {
  canPlayMediaItem,
  isLibraryDisplayableMediaItem,
} from "../src/utils/mediaPlaybackSafety.ts";

test("canPlayMediaItem accepts real video files", () => {
  assert.equal(
    canPlayMediaItem({ media_type: "movie", file_path: "E:\\Library\\movie.mp4" }),
    true,
  );
});

test("canPlayMediaItem rejects generated chapter images and empty paths", () => {
  assert.equal(
    canPlayMediaItem({ media_type: "photo", file_path: "E:\\Videos\\scene_chapters\\chapter_0001.jpg" }),
    false,
  );
  assert.equal(
    canPlayMediaItem({ media_type: "movie", file_path: "" }),
    false,
  );
});

test("isLibraryDisplayableMediaItem hides generated chapter images from the library", () => {
  assert.equal(
    isLibraryDisplayableMediaItem({
      media_type: "photo",
      file_path: "E:\\Videos\\scene_chapters\\chapter_0001.jpg",
    }),
    false,
  );
  assert.equal(
    isLibraryDisplayableMediaItem({
      media_type: "movie",
      file_path: "E:\\Videos\\scene.mp4",
    }),
    true,
  );
});
