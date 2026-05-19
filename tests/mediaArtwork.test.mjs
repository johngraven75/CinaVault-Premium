import test from "node:test";
import assert from "node:assert/strict";

import {
  pickBackdropImagePath,
  pickPosterImagePath,
} from "../src/utils/mediaArtwork.ts";

test("pickPosterImagePath prefers poster art but uses backdrop when poster is missing", () => {
  assert.equal(
    pickPosterImagePath({
      poster_path: "E:\\Library\\Movie-poster.jpg",
      backdrop_path: "E:\\Library\\Movie-backdrop.jpg",
    }),
    "E:\\Library\\Movie-poster.jpg",
  );

  assert.equal(
    pickPosterImagePath({
      poster_path: "",
      backdrop_path: "E:\\Library\\Movie-backdrop.jpg",
    }),
    "E:\\Library\\Movie-backdrop.jpg",
  );
});

test("pickBackdropImagePath uses poster art when backdrop is missing", () => {
  assert.equal(
    pickBackdropImagePath({
      poster_path: "E:\\Library\\Movie-poster.jpg",
      backdrop_path: null,
    }),
    "E:\\Library\\Movie-poster.jpg",
  );
});
