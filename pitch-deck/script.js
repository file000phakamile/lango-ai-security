// Lango — AI Data Guard, pitch deck navigation.
//
// Vanilla JS, deliberately — this deck is ten static slides toggling
// visibility, which doesn't need a slide-deck library (reveal.js and
// similar exist for far more than this: fragment animations, themes,
// speaker notes, remote control, plugins). Pulling one in would be new
// dependency weight for a problem "show one <section> at a time, disable
// buttons at the edges" solves in under 60 lines. See Questions.md for
// this call stated explicitly, per the task's "justify rather than
// default to a library" instruction.
(function () {
  "use strict";

  const slides = Array.from(document.querySelectorAll(".slide"));
  const total = slides.length;
  const counter = document.getElementById("counter");
  const prevBtn = document.getElementById("prevBtn");
  const nextBtn = document.getElementById("nextBtn");

  let current = 1; // 1-indexed, matches each slide's data-slide attribute and the on-screen counter

  function render() {
    slides.forEach((slide) => {
      const n = Number(slide.dataset.slide);
      slide.classList.toggle("active", n === current);
    });
    counter.textContent = `${current} / ${total}`;
    prevBtn.disabled = current === 1;
    nextBtn.disabled = current === total;
  }

  function goTo(n) {
    current = Math.min(Math.max(n, 1), total);
    render();
  }

  function next() {
    goTo(current + 1);
  }

  function prev() {
    goTo(current - 1);
  }

  prevBtn.addEventListener("click", prev);
  nextBtn.addEventListener("click", next);

  // Left/Right arrow keys, not Up/Down — the near-universal convention
  // for slide decks (PowerPoint, Keynote, Google Slides, reveal.js all
  // default to horizontal arrow-key navigation), and Up/Down risk
  // conflicting with a reader's instinct to scroll, even though this
  // deck itself doesn't scroll. Not bound to both to keep one
  // unambiguous, predictable mapping rather than two overlapping ones.
  document.addEventListener("keydown", (e) => {
    if (e.key === "ArrowRight") {
      next();
    } else if (e.key === "ArrowLeft") {
      prev();
    }
  });

  render();
})();
