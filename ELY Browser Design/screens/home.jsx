// home.jsx — Home / New Tab
const HomeScreen = () => (
  <ElyShell wallpaper="dawn" sidebarProps={{ activeId:'home', mode:'home' }}>
    <TopBar url="Search ELY or type a command…"/>
    <div className="ely-scroll" style={{flex:1, overflowY:'auto', padding:'48px 64px 28px'}}>
      <div style={{textAlign:'center', maxWidth: 880, margin:'0 auto'}}>
        <div style={{display:'inline-flex', alignItems:'center', gap:8, color:'var(--ely-ink-3)', fontSize:13, marginBottom:14}}>
          <span style={{color:'#e89a6e'}}><I.Sunrise size={16}/></span>
          Good morning, Alex
        </div>
        <h1 className="ely-serif" style={{fontSize: 64, lineHeight: 1.05, margin:'0 0 28px', fontWeight:400, letterSpacing:'-0.02em', color:'var(--ely-ink)'}}>
          Where focus finds flow.
        </h1>
        <div style={{position:'relative', maxWidth: 640, margin:'0 auto'}}>
          <div style={{height:54, background:'rgba(255,255,255,0.85)', borderRadius:14, boxShadow:'var(--ely-shadow-card)', display:'flex', alignItems:'center', padding:'0 16px', gap:12}}>
            <I.Search size={16}/>
            <span style={{color:'var(--ely-ink-4)', fontSize:14, flex:1, textAlign:'left'}}>Search the web or ELY</span>
            <button className="ely-iconbtn" style={{background:'rgba(40,30,20,0.04)'}}><I.ArrowRight size={14}/></button>
          </div>
        </div>
        <div style={{display:'flex', justifyContent:'center', gap:8, marginTop:14}}>
          {[['Search Tabs', <I.Search size={12}/>], ['Switch Workspace', <I.Layers size={12}/>], ['Open Notion', <Brand.Notion s={12}/>]].map(([t,i],idx)=>(
            <span key={idx} style={{display:'inline-flex',alignItems:'center',gap:6, padding:'6px 12px', borderRadius:999, background:'rgba(255,255,255,0.55)', boxShadow:'var(--ely-shadow-soft)', fontSize:12, color:'var(--ely-ink-2)'}}>
              {i}{t}
            </span>
          ))}
        </div>
      </div>

      {/* favorites */}
      <div style={{marginTop:48}}>
        <div style={{display:'flex', alignItems:'center', gap:6, color:'var(--ely-ink-3)', fontSize:12.5, marginBottom:12, fontWeight:500}}>
          Favorites <I.ChevronDown size={12}/>
        </div>
        <div style={{display:'grid', gridTemplateColumns:'repeat(7, 1fr)', gap:10}}>
          {[
            [<Brand.Notion s={28}/>, 'Notion'],
            [<Brand.YT s={28}/>, 'YouTube'],
            [<Brand.Dribbble s={28}/>, 'Dribbble'],
            [<Brand.X s={28}/>, 'X'],
            [<Brand.Vercel s={28}/>, 'Vercel'],
            [<Brand.Behance s={28}/>, 'Behance'],
          ].map(([g,n], i)=>(
            <div key={i} className="ely-card" style={{height:96, display:'flex', flexDirection:'column', alignItems:'center', justifyContent:'center', gap:8}}>
              {g}
              <div style={{fontSize:11.5, color:'var(--ely-ink-3)'}}>{n}</div>
            </div>
          ))}
          <div className="ely-card" style={{height:96, display:'flex', alignItems:'center', justifyContent:'center', color:'var(--ely-ink-4)', background:'rgba(255,255,255,0.5)'}}>
            <I.Plus size={18}/>
          </div>
        </div>
      </div>

      {/* lower row */}
      <div style={{display:'grid', gridTemplateColumns:'1.6fr 1fr', gap:12, marginTop:16}}>
        <div className="ely-card" style={{padding:16, display:'grid', gridTemplateColumns:'1fr 240px', gap:16}}>
          <div>
            <div style={{fontSize:13, fontWeight:500, marginBottom:14}}>Continue where you left off</div>
            <div style={{display:'flex', flexDirection:'column', gap:10}}>
              {[
                [<Brand.Figma s={26}/>, 'Design System – Figma', 'figma.com', '2 min ago'],
                [<div style={{width:26,height:26,borderRadius:7,background:'linear-gradient(135deg,#7c6cf7,#1d1c1a)'}}/>, 'Roadmap 2024', 'linear.app', '1 hour ago'],
                [<Icon size={26}><circle cx="12" cy="12" r="9" stroke="#6b6660" strokeWidth="1.4" fill="none"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" stroke="#6b6660" strokeWidth="1.4" fill="none"/></Icon>, 'Marketing Site', 'elybrowser.com', '3 hours ago'],
              ].map(([icon, name, host, when], i)=>(
                <div key={i} style={{display:'flex', alignItems:'center', gap:12, padding:'4px 0'}}>
                  {icon}
                  <div style={{flex:1, minWidth:0}}>
                    <div style={{fontSize:13, fontWeight:500, color:'var(--ely-ink)'}}>{name}</div>
                    <div style={{fontSize:11.5, color:'var(--ely-ink-4)'}}>{host} · {when}</div>
                  </div>
                </div>
              ))}
            </div>
          </div>
          <div style={{position:'relative', borderRadius:12, overflow:'hidden', background:'linear-gradient(135deg,#dcc4b6,#9aa5c4)', minHeight:200}}>
            <div style={{position:'absolute', inset:0, background:`
              radial-gradient(60% 40% at 50% 30%, rgba(255,200,210,0.7), transparent),
              radial-gradient(80% 60% at 30% 80%, rgba(120,140,180,0.6), transparent)
            `}}/>
            <div style={{position:'absolute', top:8, right:8, background:'rgba(0,0,0,0.35)', backdropFilter:'blur(8px)', color:'#fff', padding:'4px 10px', borderRadius:999, fontSize:10.5, display:'flex', alignItems:'center', gap:6}}>
              Reading List <span style={{opacity:.6}}>·</span> 5
            </div>
            <div style={{position:'absolute', bottom:0, left:0, right:0, padding:14, color:'#fff', textShadow:'0 1px 8px rgba(0,0,0,0.4)'}}>
              <div className="ely-serif" style={{fontSize:18, lineHeight:1.15, fontWeight:500}}>The aesthetics of<br/>calm technology</div>
              <div style={{fontSize:11, opacity:.85, marginTop:6}}>Inside ELY Journal · 6 min read</div>
            </div>
          </div>
        </div>

        <div className="ely-card" style={{padding:16}}>
          <div style={{fontSize:13, fontWeight:500, marginBottom:12}}>Recent activity</div>
          <div style={{display:'flex', flexDirection:'column', gap:12}}>
            {[
              ['viewed', 'Pricing Page', 'elybrowser.com/pricing', '2m', <Icon size={14}><circle cx="12" cy="12" r="9"/><path d="M3 12h18"/></Icon>],
              ['opened', 'Style Guide', 'figma.com/file/12345', '1h', <Brand.Figma s={14}/>],
              ['commented on', 'Roadmap 2024', 'linear.app/roadmap', '3h', <Brand.Linear s={14}/>],
            ].map(([verb, name, url, when, ic], i)=>(
              <div key={i} style={{display:'flex', alignItems:'flex-start', gap:10}}>
                <div style={{width:24, height:24, borderRadius:6, background:'rgba(255,255,255,0.8)', boxShadow:'var(--ely-shadow-soft)', display:'flex',alignItems:'center',justifyContent:'center'}}>{ic}</div>
                <div style={{flex:1, minWidth:0}}>
                  <div style={{fontSize:11.5, color:'var(--ely-ink-3)'}}>You {verb}</div>
                  <div style={{fontSize:13, fontWeight:500}}>{name}</div>
                  <div style={{fontSize:10.5, color:'var(--ely-ink-4)', marginTop:1}}>{url}</div>
                </div>
                <div style={{fontSize:10.5, color:'var(--ely-ink-4)'}}>{when}</div>
              </div>
            ))}
          </div>
          <div style={{marginTop:14, paddingTop:10, borderTop:'1px solid var(--ely-divider)', fontSize:11.5, color:'var(--ely-ink-3)', display:'inline-flex', alignItems:'center', gap:4}}>
            View all history <I.ArrowRight size={11}/>
          </div>
        </div>
      </div>
    </div>
  </ElyShell>
);
window.HomeScreen = HomeScreen;
