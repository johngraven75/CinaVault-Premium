import { motion } from 'framer-motion';
import {
  Airplay,
  Clapperboard,
  Cloud,
  Film,
  Gauge,
  Home,
  Library,
  Play,
  Radio,
  Search,
  Settings,
  Sparkles,
  Tv,
  Users,
} from 'lucide-react';

const nav = [
  [Home, 'Home'],
  [Library, 'Library'],
  [Clapperboard, 'Movies'],
  [Tv, 'Shows'],
  [Radio, 'Live TV'],
  [Airplay, 'Casting'],
  [Users, 'Users'],
  [Gauge, 'Activity'],
] as const;

const cards = [
  { title: 'Continue Watching', subtitle: 'Resume instantly across every device', icon: Play },
  { title: 'Your Library', subtitle: 'Fast local and cloud-backed discovery', icon: Film },
  { title: 'Cloud Ready', subtitle: 'Secure streaming designed to scale', icon: Cloud },
];

export function App() {
  return (
    <div className="app-shell">
      <div className="aurora aurora-one" />
      <div className="aurora aurora-two" />

      <aside className="sidebar glass">
        <motion.div className="brand" initial={{ opacity: 0, x: -18 }} animate={{ opacity: 1, x: 0 }}>
          <div className="brand-mark"><Sparkles size={21} /></div>
          <div><strong>CinaVault</strong><span>ReImagined</span></div>
        </motion.div>

        <nav>
          {nav.map(([Icon, label], index) => (
            <motion.button
              className={index === 0 ? 'nav-item active' : 'nav-item'}
              key={label}
              whileHover={{ x: 5, scale: 1.015 }}
              whileTap={{ scale: 0.97 }}
            >
              <Icon size={19} /><span>{label}</span>
            </motion.button>
          ))}
        </nav>

        <button className="nav-item settings"><Settings size={19} /><span>Settings</span></button>
      </aside>

      <main>
        <header className="topbar">
          <div className="search glass"><Search size={18} /><input aria-label="Search" placeholder="Search movies, shows, people and collections" /></div>
          <div className="status-pill"><span /> ReImagined v0.0.1</div>
        </header>

        <motion.section className="hero" initial={{ opacity: 0, y: 24 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.65 }}>
          <div className="hero-copy">
            <span className="eyebrow"><Sparkles size={15} /> A faster media universe</span>
            <h1>Your cinema.<br/><em>Reimagined.</em></h1>
            <p>A fluid Windows media experience for local libraries, cloud streaming, casting, live television and every screen you own.</p>
            <div className="hero-actions">
              <motion.button className="primary" whileHover={{ scale: 1.04 }} whileTap={{ scale: 0.97 }}><Play size={18} fill="currentColor" /> Explore library</motion.button>
              <motion.button className="secondary glass" whileHover={{ scale: 1.04 }} whileTap={{ scale: 0.97 }}><Airplay size={18} /> Connect a device</motion.button>
            </div>
          </div>
          <motion.div className="orbital" animate={{ rotate: 360 }} transition={{ duration: 32, repeat: Infinity, ease: 'linear' }}>
            <div className="orbital-core"><Film size={42} /></div>
          </motion.div>
        </motion.section>

        <section className="feature-grid">
          {cards.map(({ title, subtitle, icon: Icon }, index) => (
            <motion.article className="feature-card glass" key={title} initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.18 + index * 0.1 }} whileHover={{ y: -7 }}>
              <div className="feature-icon"><Icon size={22} /></div>
              <div><h2>{title}</h2><p>{subtitle}</p></div>
              <span className="card-arrow">→</span>
            </motion.article>
          ))}
        </section>

        <section className="rail-header"><div><span>Featured tonight</span><h2>Made for your screen</h2></div><button>View all</button></section>
        <section className="media-rail">
          {['Neon Horizon', 'Last Signal', 'The Deep Archive', 'Midnight Circuit'].map((title, index) => (
            <motion.article className={`poster poster-${index + 1}`} key={title} whileHover={{ scale: 1.035, y: -6 }}>
              <div className="poster-overlay"><button aria-label={`Play ${title}`}><Play size={20} fill="currentColor" /></button><div><strong>{title}</strong><span>4K • Dolby Atmos</span></div></div>
            </motion.article>
          ))}
        </section>
      </main>
    </div>
  );
}
