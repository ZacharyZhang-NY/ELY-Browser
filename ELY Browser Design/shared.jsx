// shared.jsx — icons + sidebar + browser shell shared across screens
// All Lucide-style 1.5px stroke SVG.

const Icon = ({ d, size = 16, fill = 'none', stroke = 'currentColor', sw = 1.5, children, style }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill={fill} stroke={stroke}
    strokeWidth={sw} strokeLinecap="round" strokeLinejoin="round" style={style}>
    {d ? <path d={d} /> : children}
  </svg>
);

const I = {
  ArrowLeft:  (p)=> <Icon size={p?.size||16} d="M19 12H5M12 19l-7-7 7-7"/>,
  ArrowRight: (p)=> <Icon size={p?.size||16} d="M5 12h14M12 5l7 7-7 7"/>,
  ArrowUpRight:(p)=> <Icon size={p?.size||14} d="M7 17 17 7M7 7h10v10"/>,
  Plus:       (p)=> <Icon size={p?.size||14} d="M12 5v14M5 12h14"/>,
  X:          (p)=> <Icon size={p?.size||14} d="M18 6 6 18M6 6l12 12"/>,
  Search:     (p)=> <Icon size={p?.size||14}><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></Icon>,
  Star:       (p)=> <Icon size={p?.size||14} d="M12 17.3l-5.4 3 1-6-4.3-4.2 6-.9L12 3.5l2.7 5.7 6 .9-4.3 4.2 1 6z"/>,
  Sparkle:    (p)=> <Icon size={p?.size||14} d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1"/>,
  Sun:        (p)=> <Icon size={p?.size||16}><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></Icon>,
  Moon:       (p)=> <Icon size={p?.size||16} d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/>,
  Sunrise:    (p)=> <Icon size={p?.size||14} d="M17 18a5 5 0 0 0-10 0M12 9V2M4.2 10.2l1.4 1.4M19.8 10.2l-1.4 1.4M2 18h2M20 18h2M22 22H2M8 6l4-4 4 4"/>,
  Settings:   (p)=> <Icon size={p?.size||16}><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1A1.7 1.7 0 0 0 4.6 9a1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></Icon>,
  Menu:       (p)=> <Icon size={p?.size||16} d="M4 6h16M4 12h16M4 18h16"/>,
  Filters:    (p)=> <Icon size={p?.size||14} d="M3 6h13M3 12h9M3 18h6M19 4v6M22 7h-6M16 14v6M19 17h-6"/>,
  Bookmark:   (p)=> <Icon size={p?.size||14} d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/>,
  Copy:       (p)=> <Icon size={p?.size||16}><rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h10"/></Icon>,
  Download:   (p)=> <Icon size={p?.size||16} d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/>,
  ChevronDown:(p)=> <Icon size={p?.size||14} d="m6 9 6 6 6-6"/>,
  ChevronRight:(p)=> <Icon size={p?.size||14} d="m9 6 6 6-6 6"/>,
  ChevronUp:  (p)=> <Icon size={p?.size||14} d="m6 15 6-6 6 6"/>,
  Layers:     (p)=> <Icon size={p?.size||16} d="M12 2 2 7l10 5 10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>,
  Home:       (p)=> <Icon size={p?.size||16} d="M3 9.5 12 3l9 6.5V21a1 1 0 0 1-1 1h-5v-7H9v7H4a1 1 0 0 1-1-1z"/>,
  Calendar:   (p)=> <Icon size={p?.size||16}><rect x="3" y="4" width="18" height="18" rx="2"/><path d="M16 2v4M8 2v4M3 10h18"/></Icon>,
  Globe:      (p)=> <Icon size={p?.size||14}><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/></Icon>,
  Lock:       (p)=> <Icon size={p?.size||14}><rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11V7a4 4 0 1 1 8 0v4"/></Icon>,
  Sidebar:    (p)=> <Icon size={p?.size||16}><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/></Icon>,
  Split:      (p)=> <Icon size={p?.size||16}><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M12 4v16"/></Icon>,
  Cmd:        (p)=> <Icon size={p?.size||14} d="M9 9V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3z"/>,
  Reload:     (p)=> <Icon size={p?.size||14} d="M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5"/>,
  Pin:        (p)=> <Icon size={p?.size||14} d="M12 17v5M9 3h6l-1 4 3 4H7l3-4z"/>,
  Tab:        (p)=> <Icon size={p?.size||14} d="M3 12h13l-3-3M3 12l3 3M21 5v14"/>,
  Inbox:      (p)=> <Icon size={p?.size||14} d="M22 12h-6l-2 3h-4l-2-3H2M5.5 5h13l3 7v6a2 2 0 0 1-2 2H4.5a2 2 0 0 1-2-2v-6z"/>,
  Trash:      (p)=> <Icon size={p?.size||14} d="M3 6h18M19 6l-1.5 14a2 2 0 0 1-2 2H8.5a2 2 0 0 1-2-2L5 6M10 11v6M14 11v6M9 6V4a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v2"/>,
  Eye:        (p)=> <Icon size={p?.size||14}><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7z"/><circle cx="12" cy="12" r="3"/></Icon>,
  Mic:        (p)=> <Icon size={p?.size||14} d="M12 2a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3zM5 11a7 7 0 0 0 14 0M12 18v4"/>,
  Lightning:  (p)=> <Icon size={p?.size||14} d="M13 2 4 14h7l-1 8 9-12h-7z"/>,
  Shield:     (p)=> <Icon size={p?.size||14} d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>,
  Cloud:      (p)=> <Icon size={p?.size||14} d="M17 18a5 5 0 0 0-1-9.9A7 7 0 1 0 5 16h12z"/>,
  Plug:       (p)=> <Icon size={p?.size||14} d="M9 2v4M15 2v4M7 6h10v4a5 5 0 0 1-10 0zM12 15v7"/>,
  Heart:      (p)=> <Icon size={p?.size||14} d="M12 21s-7-4.5-9-9a5 5 0 0 1 9-3 5 5 0 0 1 9 3c-2 4.5-9 9-9 9z"/>,
  Check:      (p)=> <Icon size={p?.size||14} d="m5 12 5 5 9-11"/>,
  Dot:        (p)=> <Icon size={p?.size||4} fill="currentColor"><circle cx="12" cy="12" r="6"/></Icon>,
  Key:        (p)=> <Icon size={p?.size||14}><circle cx="8" cy="15" r="4"/><path d="m11 12 9-9 1 4-3 1 1 3-4 1z"/></Icon>,
  Devices:    (p)=> <Icon size={p?.size||14}><rect x="3" y="5" width="13" height="11" rx="1.5"/><rect x="14" y="11" width="7" height="9" rx="1.5"/></Icon>,
  User:       (p)=> <Icon size={p?.size||14}><circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/></Icon>,
  Bell:       (p)=> <Icon size={p?.size||14} d="M6 8a6 6 0 1 1 12 0c0 7 3 9 3 9H3s3-2 3-9M10 21a2 2 0 0 0 4 0"/>,
  Palette:    (p)=> <Icon size={p?.size||14} d="M12 22a10 10 0 1 1 10-10c0 2-2 3-4 3h-2a2 2 0 0 0-2 2v2c0 2-1 3-2 3z"/>,
  Code:       (p)=> <Icon size={p?.size||14} d="m8 6-6 6 6 6M16 6l6 6-6 6M14 4l-4 16"/>,
  Filter:     (p)=> <Icon size={p?.size||14} d="M22 3H2l8 9.5V21l4-2v-6.5z"/>,
};

/* ===== little brand glyphs (faux app icons) ===== */
const Brand = {
  Notion: ({s=22}) => (
    <div style={{width:s, height:s, borderRadius:5, background:'#fff', display:'inline-flex',alignItems:'center',justifyContent:'center',
      boxShadow:'0 0 0 1px rgba(0,0,0,0.08)', fontFamily:'serif', fontWeight:700, fontSize:s*0.65, color:'#111'}}>N</div>
  ),
  YT: ({s=22}) => (
    <div style={{width:s, height:s, borderRadius:5, background:'#FF3B2D', display:'inline-flex',alignItems:'center',justifyContent:'center', color:'#fff'}}>
      <svg width={s*0.55} height={s*0.55} viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
    </div>
  ),
  Dribbble: ({s=22}) => (
    <div style={{width:s, height:s, borderRadius:'50%', background:'#ea4c89', display:'inline-flex',alignItems:'center',justifyContent:'center', color:'#fff', fontWeight:800, fontSize: s*0.55, fontStyle:'italic'}}>·</div>
  ),
  X: ({s=22}) => (
    <div style={{width:s, height:s, borderRadius:5, background:'#000', display:'inline-flex',alignItems:'center',justifyContent:'center', color:'#fff', fontWeight:700, fontSize: s*0.55}}>𝕏</div>
  ),
  Vercel: ({s=22}) => (
    <div style={{width:s, height:s, borderRadius:5, background:'#fff', display:'inline-flex',alignItems:'center',justifyContent:'center', boxShadow:'0 0 0 1px rgba(0,0,0,0.08)'}}>
      <svg width={s*0.65} height={s*0.65} viewBox="0 0 24 24" fill="#000"><path d="M12 4 22 20H2z"/></svg>
    </div>
  ),
  Behance: ({s=22}) => (
    <div style={{width:s, height:s, borderRadius:5, background:'#1769ff', display:'inline-flex',alignItems:'center',justifyContent:'center', color:'#fff', fontWeight:800, fontSize: s*0.55, fontFamily:'serif'}}>Bē</div>
  ),
  Figma: ({s=18}) => (
    <div style={{width:s, height:s, position:'relative'}}>
      <svg width={s} height={s} viewBox="0 0 24 24">
        <path fill="#F24E1E" d="M8 2h4v6H8a3 3 0 0 1 0-6z"/>
        <path fill="#A259FF" d="M12 2h4a3 3 0 0 1 0 6h-4z"/>
        <path fill="#F24E1E" d="M8 8h4v6H8a3 3 0 0 1 0-6z" opacity="0"/>
        <path fill="#1ABCFE" d="M12 8h.5a3 3 0 1 1-.5 6V8z"/>
        <path fill="#0ACF83" d="M8 14h4v3a3 3 0 0 1-6 0 3 3 0 0 1 2-3z"/>
        <path fill="#FF7262" d="M8 8h4v6H8a3 3 0 0 1 0-6z"/>
      </svg>
    </div>
  ),
  Slack: ({s=18}) => (
    <svg width={s} height={s} viewBox="0 0 24 24"><path fill="#36c5f0" d="M5 14a2 2 0 1 0-2-2h2zm1 0a2 2 0 1 1 4 0v5a2 2 0 1 1-4 0z"/><path fill="#2eb67d" d="M10 5a2 2 0 1 0 2-2v2zm0 1a2 2 0 1 1 0 4H5a2 2 0 1 1 0-4z"/><path fill="#ecb22e" d="M19 10a2 2 0 1 0 2 2h-2zm-1 0a2 2 0 1 1-4 0V5a2 2 0 1 1 4 0z"/><path fill="#e01e5a" d="M14 19a2 2 0 1 0-2 2v-2zm0-1a2 2 0 1 1 0-4h5a2 2 0 1 1 0 4z"/></svg>
  ),
  GitHub: ({s=18}) => (
    <svg width={s} height={s} viewBox="0 0 24 24" fill="#1d1c1a"><path d="M12 2a10 10 0 0 0-3.16 19.49c.5.09.66-.22.66-.48v-1.7c-2.78.6-3.37-1.34-3.37-1.34-.46-1.16-1.11-1.46-1.11-1.46-.91-.62.07-.6.07-.6 1 .07 1.53 1.03 1.53 1.03.89 1.52 2.34 1.08 2.91.83.09-.65.35-1.08.63-1.33-2.22-.25-4.55-1.11-4.55-4.94 0-1.09.39-1.98 1.03-2.68-.1-.25-.45-1.27.1-2.65 0 0 .84-.27 2.75 1.02a9.6 9.6 0 0 1 5 0c1.91-1.29 2.75-1.02 2.75-1.02.55 1.38.2 2.4.1 2.65.64.7 1.03 1.59 1.03 2.68 0 3.84-2.34 4.69-4.57 4.94.36.31.68.92.68 1.85v2.74c0 .27.16.58.67.48A10 10 0 0 0 12 2z"/></svg>
  ),
  Linear: ({s=18}) => (
    <div style={{width:s, height:s, borderRadius:'50%', background:'linear-gradient(135deg,#5e6ad2,#1d1c1a)', display:'inline-flex',alignItems:'center',justifyContent:'center', color:'#fff', fontSize:s*0.5, fontWeight:600, letterSpacing:'-0.05em'}}>L</div>
  ),
};

/* ============== SIDEBAR ============== */
const Sidebar = ({ activeId = 'home', mode = 'home', tabs, withTrafficTitle = true, workspace='Work', accentEmoji='🌸' }) => {
  const defaultTabs = [
    { id:'figma', label:'Design System – Figma', icon:<Brand.Figma s={16}/>, closable:true },
    { id:'roadmap', label:'Roadmap 2024', icon:<div style={{width:16,height:16,borderRadius:4, background:'linear-gradient(135deg,#7c6cf7,#ec6a8e)'}}/> },
    { id:'mkt', label:'Marketing Site', icon:<Icon size={16} stroke="#6b6660"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18"/></Icon> },
  ];
  const t = tabs || defaultTabs;
  return (
    <aside className="ely-sidebar">
      {withTrafficTitle && (
        <div className="ely-traffic">
          <span className="dot red"/><span className="dot yellow"/><span className="dot green"/>
          <span className="title">ELY Browser <I.ChevronDown size={12}/></span>
        </div>
      )}
      <div className="ely-ws-row">
        <div className="ely-ws-tile">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6">
            <rect x="3" y="4" width="8" height="16" rx="1.5"/>
            <rect x="13" y="4" width="8" height="7" rx="1.5"/>
            <rect x="13" y="13" width="8" height="7" rx="1.5"/>
          </svg>
        </div>
        <div className="ely-ws-pick">
          <span className="ws-glyph">{accentEmoji}</span>
          <span className="grow">{workspace}</span>
          <I.ChevronDown size={12}/>
        </div>
        <div className="ely-ws-add"><I.Plus size={14}/></div>
      </div>

      <div className="ely-nav">
        <div className={'ely-nav-item' + (activeId==='home'?' active':'')}>
          <span className="ico"><I.Home size={16}/></span>
          <span className="label">Home</span>
        </div>
        <div className="ely-nav-item">
          <span className="ico-tile" style={{color:'#f24e1e'}}>31</span>
          <span className="label">Calendar</span>
        </div>
        <div className="ely-nav-item">
          <span className="ico-tile"><Brand.Slack s={12}/></span>
          <span className="label">Slack</span>
          <span className="badge">12</span>
        </div>
        <div className="ely-nav-item">
          <span className="ico-tile"><Brand.GitHub s={12}/></span>
          <span className="label">GitHub</span>
        </div>
        <div className="ely-nav-item">
          <span className="ico-tile"><Brand.Linear s={12}/></span>
          <span className="label">Linear</span>
          <span className="badge">3</span>
        </div>
      </div>

      <div className="ely-sec-label"><I.Pin size={11}/> Pinned</div>
      <div className="ely-nav">
        <div className="ely-nav-item">
          <span className="ico"><Brand.Figma s={16}/></span>
          <span className="label">Design System – Figma</span>
        </div>
        <div className={'ely-nav-item' + (activeId==='ely'?' active':'')}>
          <span className="ico"><div style={{width:16,height:16,borderRadius:'50%', background:'linear-gradient(135deg,#ec6a8e,#c96442)'}}/></span>
          <span className="label">ELY Browser – Home</span>
          <span className="dot-unread"/>
        </div>
        <div className="ely-nav-item">
          <span className="ico"><Icon size={16} stroke="#6b6660" d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8zM14 2v6h6M9 13h6M9 17h6M9 9h2"/></span>
          <span className="label">Release Notes</span>
        </div>
        <div className="ely-nav-item">
          <span className="ico"><div style={{width:16,height:16,borderRadius:4, background:'linear-gradient(135deg,#7c6cf7,#ec6a8e)'}}/></span>
          <span className="label">Roadmap 2024</span>
          <span className="dot-unread"/>
        </div>
        <div className="ely-nav-item">
          <span className="ico"><Icon size={16} stroke="#6b6660"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18"/></Icon></span>
          <span className="label">Marketing Site</span>
        </div>
      </div>

      <div className="ely-sec-label"><I.Tab size={11}/> Tabs · {t.length}</div>
      <div className="ely-nav">
        {t.map(tab => (
          <div key={tab.id} className={'ely-nav-item' + (mode==='browse' && tab.id==='figma'?' active':'') + (tab.active?' active':'')}>
            <span className="ico">{tab.icon}</span>
            <span className="label">{tab.label}</span>
            {tab.audio && <span className="ico" style={{color:'var(--ely-accent)'}}><I.Lightning size={12}/></span>}
            <span className="close"><I.X size={12}/></span>
          </div>
        ))}
        <div className="ely-nav-item" style={{color:'var(--ely-ink-3)'}}>
          <span className="ico"><I.Plus size={14}/></span>
          <span className="label">New Tab</span>
        </div>
      </div>

      <div className="ely-sidebar-foot">
        <div className="ely-nav-item">
          <span className="ico"><I.Settings size={15}/></span>
          <span className="label">Settings</span>
        </div>
        <div className="ely-profile-row">
          <div className="ely-avatar">A</div>
          <span className="name">Alex</span>
          <I.ChevronDown size={12}/>
        </div>
      </div>
    </aside>
  );
};

/* Browser shell that hosts a sidebar + main area */
const ElyShell = ({ wallpaper='dawn', children, sidebarProps={}, fakeTraffic=true }) => (
  <div className="ely-window">
    <div className={'ely-wallpaper ely-wp-' + wallpaper}/>
    <div className="ely-shell">
      <Sidebar {...sidebarProps} withTrafficTitle={fakeTraffic}/>
      <main className="ely-main">{children}</main>
    </div>
  </div>
);

/* topbar */
const TopBar = ({ url, host, path, secure=true, right, canBack=true, canForward=false }) => (
  <div className="ely-topbar">
    <button className={'ely-iconbtn'+(canBack?'':' disabled')}><I.ArrowLeft size={16}/></button>
    <button className={'ely-iconbtn'+(canForward?'':' disabled')}><I.ArrowRight size={16}/></button>
    <div className="ely-omnibar">
      {secure ? <I.Lock size={14}/> : <I.Globe size={14}/>}
      {host ? (
        <>
          <span className="url-host">{host}</span>
          {path && <span className="url-path">{path}</span>}
        </>
      ) : (
        <span>{url || 'Search ELY or type a command…'}</span>
      )}
      <span className="right">
        <I.Filters size={14}/>
        <I.Star size={14}/>
      </span>
    </div>
    {right || (
      <>
        <button className="ely-iconbtn"><I.Copy size={16}/></button>
        <button className="ely-iconbtn"><I.Download size={16}/></button>
        <button className="ely-iconbtn"><I.Moon size={16}/></button>
        <button className="ely-iconbtn"><I.Menu size={16}/></button>
      </>
    )}
  </div>
);

window.Sidebar = Sidebar;
window.ElyShell = ElyShell;
window.TopBar = TopBar;
window.Icon = Icon;
window.I = I;
window.Brand = Brand;
