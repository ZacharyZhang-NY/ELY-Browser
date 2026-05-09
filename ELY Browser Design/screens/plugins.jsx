// plugins.jsx — Plugin Marketplace
const PluginsScreen = () => {
  const cats = ['All', 'Productivity', 'Reading', 'Privacy', 'Developer', 'AI', 'Writing', 'Design'];
  const plugins = [
    { name:'Reader+', author:'ELY Studio', desc:'Distraction-free reading mode with serif typography & ambient backgrounds.', accent:'linear-gradient(135deg,#ffd1d8,#b9c7ff)', glyph:'¶', installs:'24.5k', rating:4.9, tags:['Featured','Reading'] },
    { name:'Tab Capsules', author:'orbit.dev', desc:'Hibernate tabs into searchable capsules. Restore in one click — even months later.', accent:'linear-gradient(135deg,#c5f0db,#7c6cf7)', glyph:'◐', installs:'18.2k', rating:4.8, tags:['Productivity'] },
    { name:'Coast Privacy Shield', author:'Coast Labs', desc:'Per-site tracker counts and one-tap container tabs.', accent:'linear-gradient(135deg,#1d1c1a,#7c6cf7)', glyph:'◊', dark:true, installs:'31.0k', rating:4.7, tags:['Privacy'] },
    { name:'Quill', author:'paperhouse', desc:'Annotate any page. Notes sync to Notion, Obsidian, Bear.', accent:'linear-gradient(135deg,#fff3ee,#f4c8b8)', glyph:'✎', installs:'12.4k', rating:4.6, tags:['Writing'] },
    { name:'ELY AI · Summon', author:'ELY Studio', desc:'Ask anything about the page. Beautiful inline citations.', accent:'linear-gradient(135deg,#d6c4ff,#ffd1d8)', glyph:'✦', installs:'47.3k', rating:4.9, tags:['AI','Featured'] },
    { name:'Pomodoro Glass', author:'flux.app', desc:'Ambient timer that lives in your sidebar. Subtle, focused.', accent:'linear-gradient(135deg,#ffe5b8,#ec6a8e)', glyph:'◷', installs:'9.1k', rating:4.5, tags:['Productivity'] },
    { name:'Localhost Console', author:'devhabit', desc:'Inline preview & logs for any dev port. Toggle from omnibar.', accent:'linear-gradient(135deg,#0e0d0c,#4fb59a)', glyph:'❯_', dark:true, installs:'7.6k', rating:4.7, tags:['Developer'] },
    { name:'Lookbook', author:'studio.bento', desc:'Collect screenshots & swatches as you browse. Boards live in ELY.', accent:'linear-gradient(135deg,#ec6a8e,#7c6cf7)', glyph:'▥', installs:'14.0k', rating:4.8, tags:['Design'] },
  ];
  return (
  <ElyShell wallpaper="mint" sidebarProps={{ activeId:'plugins' }}>
    <div className="ely-topbar">
      <button className="ely-iconbtn"><I.ArrowLeft size={16}/></button>
      <button className="ely-iconbtn"><I.ArrowRight size={16}/></button>
      <div className="ely-omnibar"><I.Plug size={14}/><span className="url-host">ely://plugins</span><span className="url-path">/marketplace</span><span className="right"><I.Filters size={14}/></span></div>
      <button className="ely-iconbtn is-on"><I.Plug size={15}/></button>
      <button className="ely-iconbtn"><I.Moon size={16}/></button>
      <button className="ely-iconbtn"><I.Menu size={16}/></button>
    </div>
    <div className="ely-scroll" style={{flex:1, overflowY:'auto'}}>
      {/* Hero */}
      <div style={{padding:'40px 56px 24px', display:'grid', gridTemplateColumns:'1.4fr 1fr', gap:28, alignItems:'end'}}>
        <div>
          <div style={{fontSize:11, color:'var(--ely-ink-4)', textTransform:'uppercase', letterSpacing:'0.1em'}}>Plugins · Native to ELY</div>
          <h1 className="ely-serif" style={{fontSize:48, lineHeight:1.05, margin:'6px 0 10px', fontWeight:400, letterSpacing:'-0.02em'}}>Quiet tools.<br/>Everyday magic.</h1>
          <div style={{fontSize:13.5, color:'var(--ely-ink-2)', maxWidth:520, lineHeight:1.55}}>A small, curated marketplace built for ELY — sandboxed, theme-aware, no Chrome extension framework required. Each plugin ships with its own tweakable controls in your sidebar.</div>
          <div style={{marginTop:18, height:42, padding:'0 16px', borderRadius:12, background:'rgba(255,255,255,0.85)', boxShadow:'var(--ely-shadow-card)', display:'flex', alignItems:'center', gap:10, maxWidth:480}}>
            <I.Search size={14}/>
            <span style={{fontSize:13, color:'var(--ely-ink-4)', flex:1}}>Search 184 plugins…</span>
            <span className="ely-mono" style={{fontSize:10.5, color:'var(--ely-ink-4)'}}>⌘K</span>
          </div>
        </div>
        <div className="ely-card" style={{padding:18, background:'linear-gradient(135deg, rgba(255,255,255,0.9), rgba(214,196,255,0.55))'}}>
          <div style={{fontSize:11, color:'var(--ely-ink-3)', textTransform:'uppercase', letterSpacing:'0.1em', display:'flex',alignItems:'center', gap:6}}><I.Sparkle size={11}/> Editor's pick</div>
          <div style={{display:'flex', alignItems:'flex-start', gap:14, marginTop:12}}>
            <div style={{width:56, height:56, borderRadius:14, background:'linear-gradient(135deg,#d6c4ff,#ffd1d8)', display:'flex',alignItems:'center',justifyContent:'center', fontSize:28, color:'#fff'}}>✦</div>
            <div style={{flex:1}}>
              <div style={{fontSize:15, fontWeight:600}}>ELY AI · Summon</div>
              <div style={{fontSize:11.5, color:'var(--ely-ink-4)'}}>by ELY Studio · 47.3k installs</div>
              <div style={{fontSize:12.5, color:'var(--ely-ink-2)', marginTop:6, lineHeight:1.4}}>Ask anything about the current page with citations.</div>
            </div>
          </div>
          <div style={{display:'flex', alignItems:'center', gap:8, marginTop:12}}>
            <span style={{padding:'7px 14px', borderRadius:8, background:'var(--ely-ink)', color:'#fff', fontSize:12, fontWeight:500}}>Install</span>
            <span style={{padding:'7px 14px', borderRadius:8, background:'rgba(255,255,255,0.7)', boxShadow:'var(--ely-shadow-soft)', fontSize:12, fontWeight:500}}>Open detail</span>
            <span style={{marginLeft:'auto', fontSize:11.5, color:'var(--ely-ink-3)'}}>⭐︎ 4.9 · 1,241 reviews</span>
          </div>
        </div>
      </div>

      {/* Cats */}
      <div style={{padding:'4px 56px 18px', display:'flex', gap:6, flexWrap:'wrap'}}>
        {cats.map((c,i)=>(
          <span key={c} style={{padding:'6px 12px', borderRadius:999, fontSize:12, background: i===0?'var(--ely-ink)':'rgba(255,255,255,0.7)', color: i===0?'#fff':'var(--ely-ink-2)', boxShadow:i===0?'none':'var(--ely-shadow-soft)', fontWeight:500}}>{c}</span>
        ))}
        <span style={{marginLeft:'auto', fontSize:11.5, color:'var(--ely-ink-3)', display:'inline-flex', alignItems:'center', gap:6}}>Sort: Most installed <I.ChevronDown size={11}/></span>
      </div>

      {/* Plugin grid */}
      <div style={{padding:'0 56px 28px', display:'grid', gridTemplateColumns:'repeat(4, 1fr)', gap:14}}>
        {plugins.map((p,i)=>(
          <div key={i} className="ely-card" style={{padding:14, display:'flex', flexDirection:'column', gap:10}}>
            <div style={{aspectRatio:'5/3', borderRadius:10, background:p.accent, display:'flex',alignItems:'center',justifyContent:'center', fontSize:36, color: p.dark?'rgba(255,255,255,0.9)':'rgba(255,255,255,0.95)', position:'relative', overflow:'hidden'}}>
              <div style={{position:'absolute', inset:0, background:'radial-gradient(60% 50% at 30% 20%, rgba(255,255,255,0.4), transparent 70%)'}}/>
              <span style={{position:'relative', textShadow:'0 2px 14px rgba(0,0,0,0.2)'}}>{p.glyph}</span>
              {p.tags.includes('Featured') && <span style={{position:'absolute', top:8, left:8, padding:'2px 8px', borderRadius:999, background:'rgba(255,255,255,0.95)', color:'var(--ely-ink)', fontSize:9.5, fontWeight:600, letterSpacing:'0.04em'}}>FEATURED</span>}
            </div>
            <div style={{display:'flex',alignItems:'flex-start',gap:8}}>
              <div style={{flex:1, minWidth:0}}>
                <div style={{fontSize:13.5, fontWeight:600}}>{p.name}</div>
                <div style={{fontSize:11, color:'var(--ely-ink-4)'}}>{p.author}</div>
              </div>
              <span style={{padding:'3px 9px', borderRadius:6, background:'rgba(40,30,20,0.05)', fontSize:11, fontWeight:500, color:'var(--ely-ink-2)'}}>Get</span>
            </div>
            <div style={{fontSize:12, color:'var(--ely-ink-3)', lineHeight:1.45, height: 50, overflow:'hidden'}}>{p.desc}</div>
            <div style={{display:'flex', alignItems:'center', gap:8, fontSize:10.5, color:'var(--ely-ink-4)', borderTop:'1px solid var(--ely-divider)', paddingTop:8, marginTop:'auto'}}>
              <span>★ {p.rating}</span>
              <span>·</span>
              <span>{p.installs} installs</span>
              <span style={{marginLeft:'auto', display:'inline-flex',alignItems:'center', gap:3, color:'#2f7d68'}}><I.Shield size={10}/> Sandboxed</span>
            </div>
          </div>
        ))}
      </div>

      {/* Plugin detail (preview / featured below) */}
      <div style={{padding:'10px 56px 40px'}}>
        <div className="ely-card" style={{padding:24, display:'grid', gridTemplateColumns:'320px 1fr', gap:24}}>
          <div>
            <div style={{aspectRatio:'4/3', borderRadius:14, background:'linear-gradient(135deg,#ffd1d8,#b9c7ff)', display:'flex',alignItems:'center',justifyContent:'center', fontSize:80, color:'#fff', textShadow:'0 4px 20px rgba(0,0,0,0.15)'}}>¶</div>
            <div style={{display:'flex', alignItems:'center', gap:10, marginTop:14}}>
              <span style={{flex:1, height:40, background:'var(--ely-ink)', color:'#fff', borderRadius:10, display:'inline-flex',alignItems:'center',justifyContent:'center', fontSize:13, fontWeight:500, gap:6}}><I.Download size={13}/> Install · Free</span>
              <span style={{width:40, height:40, background:'rgba(255,255,255,0.85)', boxShadow:'var(--ely-shadow-soft)', borderRadius:10, display:'inline-flex',alignItems:'center',justifyContent:'center'}}><I.Heart size={14}/></span>
            </div>
            <div style={{marginTop:16, fontSize:11, color:'var(--ely-ink-3)', textTransform:'uppercase', letterSpacing:'0.1em'}}>Permissions</div>
            <div style={{marginTop:8, display:'flex', flexDirection:'column', gap:6, fontSize:12}}>
              {[['Read content of pages you choose','✓'],['Store reading preferences locally','✓'],['Access network',' '],['Read browsing history',' ']].map(([t,c],i)=>(
                <div key={i} style={{display:'flex',alignItems:'center', gap:8, color: c==='✓'?'var(--ely-ink-2)':'var(--ely-ink-4)'}}>
                  <span style={{width:14, height:14, borderRadius:4, background: c==='✓'?'#e8f3ee':'rgba(40,30,20,0.05)', color:'#2f7d68', display:'inline-flex',alignItems:'center',justifyContent:'center', fontSize:10}}>{c}</span>
                  {t}
                </div>
              ))}
            </div>
          </div>

          <div>
            <div style={{display:'flex', alignItems:'flex-start', gap:14}}>
              <div style={{flex:1}}>
                <div style={{fontSize:11, color:'var(--ely-ink-4)', textTransform:'uppercase', letterSpacing:'0.1em'}}>Reading · Featured</div>
                <h2 className="ely-serif" style={{fontSize:32, fontWeight:400, margin:'4px 0 4px', letterSpacing:'-0.02em'}}>Reader+</h2>
                <div style={{fontSize:12.5, color:'var(--ely-ink-3)'}}>by ELY Studio · v2.4 · updated 3 days ago</div>
              </div>
              <div style={{textAlign:'right'}}>
                <div style={{fontSize:18, fontWeight:600}}>★ 4.9</div>
                <div style={{fontSize:10.5, color:'var(--ely-ink-4)'}}>1,894 reviews</div>
              </div>
            </div>
            <p style={{fontSize:13.5, color:'var(--ely-ink-2)', lineHeight:1.6, marginTop:14}}>Reader+ pulls the article out of the noise. A serif type system, gentle ambient backgrounds, and a focus mode that quietly hides everything you don't need. Built native to ELY's plugin runtime — themes follow your wallpaper, settings live in the sidebar.</p>

            <div style={{display:'grid', gridTemplateColumns:'repeat(4, 1fr)', gap:8, marginTop:18}}>
              {[
                ['Installs','24,521'],['Rating','4.9 / 5'],['Size','340 kB'],['Sandboxed','Yes']
              ].map(([k,v])=>(
                <div key={k} style={{padding:'10px 12px', borderRadius:10, background:'rgba(255,255,255,0.7)', boxShadow:'var(--ely-shadow-soft)'}}>
                  <div style={{fontSize:10.5, color:'var(--ely-ink-4)', textTransform:'uppercase', letterSpacing:'0.1em'}}>{k}</div>
                  <div style={{fontSize:13, fontWeight:500, marginTop:2}}>{v}</div>
                </div>
              ))}
            </div>

            <div style={{marginTop:22}}>
              <div style={{fontSize:13, fontWeight:600, marginBottom:10}}>What it adds to ELY</div>
              <div style={{display:'flex', flexDirection:'column', gap:8}}>
                {[
                  ['Reader sidebar', 'Outline, time-to-read, table of contents — themed to your wallpaper.'],
                  ['Ambient mode', 'Subtle background motion sourced from the article\'s lead image.'],
                  ['Pocket-style queue', 'Save for later that lives in ELY\'s reading list, not a third-party app.'],
                ].map(([t,d],i)=>(
                  <div key={i} style={{display:'flex', alignItems:'flex-start', gap:10, padding:'10px 12px', borderRadius:10, background:'rgba(255,255,255,0.55)'}}>
                    <span style={{width:18, height:18, borderRadius:5, background:'rgba(201,100,66,0.12)', color:'var(--ely-accent)', display:'inline-flex',alignItems:'center',justifyContent:'center', fontSize:11, marginTop:1}}><I.Check size={11}/></span>
                    <div style={{flex:1}}>
                      <div style={{fontSize:12.5, fontWeight:500}}>{t}</div>
                      <div style={{fontSize:11.5, color:'var(--ely-ink-3)', marginTop:1, lineHeight:1.4}}>{d}</div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </ElyShell>
);};
window.PluginsScreen = PluginsScreen;
