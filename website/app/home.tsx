import * as React from "react"
import { createRoot } from "react-dom/client"
import { OmGlassReveal } from "./components/ui/liquid-glass-logo-reveal"
import LycorisSpecimen from "./components/ui/lycoris-specimen"

// Intro: the Om mark settles out of a drop of glass.
const intro = document.getElementById("om-intro")
if (intro) {
  createRoot(intro).render(
    <OmGlassReveal className="om-intro-stage" loop={false} label="Om" />,
  )
}

// Hero: a scroll-driven specimen of Om around a black bat flower.
const hero = document.getElementById("om-hero")
if (hero) {
  createRoot(hero).render(
    <LycorisSpecimen
      name="Om"
      studio="The Om project"
      year="2026"
      description="Om is a compiled systems language with Rust's ownership model and no garbage collector. No lifetime annotations, optional semicolons, default and named arguments, and string interpolation of any expression."
      specs={[
        "Native code through Cranelift",
        "No garbage collector",
        "No lifetime annotations",
        "Traits, generics, enums, match",
        "Early and experimental",
      ]}
      tagline={[
        { text: "Write" },
        { text: "less,", small: true },
        { text: "run" },
        { text: "native", small: true },
      ]}
      multilingual="run·build·check"
      ligatureWord="Ownership"
      links={[
        { label: "INSTALL OM", href: "install.html" },
        { label: "READ THE BOOK", href: "book/index.html" },
      ]}
      ink="#06070b"
      bone="#cfc9b0"
      crimson="#b98cff"
      flower="#231029"
      height="calc(100svh - 64px)"
      florets={7}
      seed={11}
    />,
  )
}
