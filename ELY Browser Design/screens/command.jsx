// command.jsx — Command Bar / Quick Switcher (overlaid on home)
const CommandScreen = () => (
  <ElyShell wallpaper="dawn" sidebarProps={{ activeId:'home', mode:'home' }}>
    <TopBar url="Search ELY or type a command…"/>
    <div style={{flex:1, position:'relative', overflow:'hidden'}}>
      {/* dimmed home behind */}
      <div style={{position:'absolute', inset:0, padding:'48px 64px', opacity:0.35, filter:'blur(4px) saturate(0.9)', pointerEvents:'none'}}>
        <div style={{textAlign:'center'}}>
          <div className="ely-serif" style={{fontSize:64, lineHeight:1.05, fontWeight:400, color:'var(--ely-ink)'}}>Where focus finds flow.</div>
        </div>
      </div>
      <div style={{position:'absolute', inset:0, background:'rgba(20,15,10,0.18)', backdropFilter:'blur(6px)'}}/>
      {/* command panel */}
      <div style={{position:'absolute', top:80, left:'50%', transform:'translateX(-50%)', width:640, background:'rgba(255,255,255,0.86)', backdropFilter:'blur(40px) saturate(160%)', borderRadius:16, boxShadow:'0 30px 80px -20px rgba(40,30,20,0.45), 0 0 0 1px rgba(255,255,255,0.6) inset, 0 1px 0 rgba(255,255,255,0.8) inset', overflow:'hidden'}}>
        <div style={{display:'flex', alignItems:'center', gap:12, padding:'16px 18px', borderBottom:'1px solid var(--ely-divider)'}}>
          <I.Search size={18}/>
          <div style={{fontSize:16, color:'var(--ely-ink)', flex:1}}>
            roa<span style={{display:'inline-block', width:1, height:18, background:'var(--ely-accent)', verticalAlign:'middle', marginLeft:1, animation:'blink 1s infinite'}}/>
          </div>
          <span style={{padding:'2px 8px', borderRadius:6, background:'rgba(40,30,20,0.06)', fontSize:10.5, color:'var(--ely-ink-3)'}}>Switcher</span>
          <span className="ely-mono" style={{fontSize:10.5, color:'var(--ely-ink-4)'}}>esc</span>
        </div>

        <div style={{maxHeight:440, overflowY:'auto'}}>
          {/* Section: Open tabs */}
          <CmdSection label="Open tabs">
            <CmdRow icon={<Brand.Figma s={16}/>} title="Style Guide — Figma" hint="figma.com" right="⌘1" />
            <CmdRow active icon={<Brand.Linear s={16}/>} title="Roadmap 2024 — Linear" hint="linear.app · 6 issues open" right="⌘2 · ↵" />
            <CmdRow icon={<div style={{width:16,height:16, borderRadius:4, background:'linear-gradient(135deg,#7c6cf7,#ec6a8e)'}}/>} title="ELY Roadmap doc — Notion" hint="notion.so" right="⌘3" />
          </CmdSection>

          <CmdSection label="History">
            <CmdRow icon={<Icon size={14}><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18"/></Icon>} title="Roadmap.app · 2024 update" hint="roadmap.app · 1d ago" />
            <CmdRow icon={<Brand.GitHub s={14}/>} title="elybrowser/core · roadmap.md" hint="github.com · 3d ago" />
          </CmdSection>

          <CmdSection label="Bookmarks">
            <CmdRow icon={<I.Bookmark size={14}/>} title="Roadmap planning template" hint="Pinned · Work" />
          </CmdSection>

          <CmdSection label="Actions">
            <CmdRow icon={<I.Layers size={14}/>} title="Switch workspace" hint="Work · Personal · Studio" right="⌘⇧W" />
            <CmdRow icon={<I.Split size={14}/>} title="Open in split view" hint="Beside current tab" right="⌘D" />
            <CmdRow icon={<I.Settings size={14}/>} title="Open Settings" hint="Appearance, Sync, Plugins…" right="⌘," />
          </CmdSection>

          <CmdSection label="Ask ELY">
            <CmdRow ai icon={<I.Sparkle size={14}/>} title='Ask: "What did we ship in the last roadmap cycle?"' hint="Searches recent tabs, history & docs" right="⌘↵"/>
          </CmdSection>
        </div>

        <div style={{display:'flex', alignItems:'center', gap:14, padding:'10px 16px', borderTop:'1px solid var(--ely-divider)', fontSize:10.5, color:'var(--ely-ink-3)', background:'rgba(255,255,255,0.55)'}}>
          <span style={{display:'inline-flex', alignItems:'center', gap:4}}><Kbd>↑</Kbd><Kbd>↓</Kbd> navigate</span>
          <span style={{display:'inline-flex', alignItems:'center', gap:4}}><Kbd>↵</Kbd> open</span>
          <span style={{display:'inline-flex', alignItems:'center', gap:4}}><Kbd>⌘↵</Kbd> open in split</span>
          <span style={{display:'inline-flex', alignItems:'center', gap:4}}><Kbd>⇥</Kbd> filter</span>
          <span style={{marginLeft:'auto', display:'inline-flex', alignItems:'center', gap:6}}>
            <I.Sparkle size={11}/> Powered by ELY
          </span>
        </div>
      </div>
    </div>

    <style>{`@keyframes blink {0%,49%{opacity:1}50%,100%{opacity:0}}`}</style>
  </ElyShell>
);

const Kbd = ({ children }) => (
  <span className="ely-mono" style={{padding:'1px 5px', minWidth:14, textAlign:'center', borderRadius:4, background:'rgba(255,255,255,0.85)', boxShadow:'0 0 0 1px var(--ely-stroke), 0 1px 0 rgba(40,30,20,0.05)', fontSize:10}}>{children}</span>
);

const CmdSection = ({ label, children }) => (
  <div style={{padding:'8px 0 4px'}}>
    <div style={{padding:'6px 16px 4px', fontSize:10, color:'var(--ely-ink-4)', textTransform:'uppercase', letterSpacing:'0.1em', fontWeight:500}}>{label}</div>
    {children}
  </div>
);

const CmdRow = ({ icon, title, hint, right, active, ai }) => (
  <div style={{display:'flex', alignItems:'center', gap:12, padding:'8px 16px', background: active?'rgba(201,100,66,0.08)': 'transparent', position:'relative'}}>
    {active && <div style={{position:'absolute', left:0, top:6, bottom:6, width:2, borderRadius:2, background:'var(--ely-accent)'}}/>}
    <div style={{width:24, height:24, borderRadius:6, background: ai?'linear-gradient(135deg,#ffd1d8,#b9c7ff)':'rgba(255,255,255,0.85)', boxShadow:'var(--ely-shadow-soft)', display:'flex',alignItems:'center',justifyContent:'center', color: ai?'#c96442':'var(--ely-ink-2)'}}>{icon}</div>
    <div style={{flex:1, minWidth:0}}>
      <div style={{fontSize:13, fontWeight: active?500:450, color:'var(--ely-ink)'}}>{title}</div>
      {hint && <div style={{fontSize:11, color:'var(--ely-ink-4)', marginTop:1}}>{hint}</div>}
    </div>
    {right && <span className="ely-mono" style={{fontSize:10.5, color:'var(--ely-ink-3)'}}>{right}</span>}
  </div>
);

window.CommandScreen = CommandScreen;
