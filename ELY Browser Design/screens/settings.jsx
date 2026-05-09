// settings.jsx — Settings page
const SettingsScreen = () => {
  const SectionLabel = ({ children, k='label' }) => k==='label' ?
    <div style={{padding:'8px 12px 4px', fontSize:10, color:'var(--ely-ink-4)', textTransform:'uppercase', letterSpacing:'0.1em', fontWeight:500}}>{children}</div>
    : <div>{children}</div>;
  const NavItem = ({ icon, label, active }) => (
    <div style={{display:'flex',alignItems:'center',gap:10, padding:'7px 12px', borderRadius:8, background: active?'rgba(255,255,255,0.85)':'transparent', boxShadow: active?'var(--ely-shadow-soft)':'none', fontSize:13, color: active?'var(--ely-ink)':'var(--ely-ink-2)'}}>
      <span style={{color:'var(--ely-ink-3)'}}>{icon}</span>{label}
    </div>
  );
  const Toggle = ({ on }) => (
    <span style={{width:32, height:18, borderRadius:999, background: on?'var(--ely-accent)':'rgba(40,30,20,0.15)', position:'relative', display:'inline-block', boxShadow:'inset 0 0 0 1px rgba(0,0,0,0.05)'}}>
      <span style={{position:'absolute', top:2, left: on?16:2, width:14, height:14, borderRadius:'50%', background:'#fff', boxShadow:'0 1px 2px rgba(0,0,0,0.2)', transition:'left .2s'}}/>
    </span>
  );
  const Row = ({ title, desc, children, last }) => (
    <div style={{display:'flex', alignItems:'center', gap:16, padding:'14px 0', borderBottom: last?'none':'1px solid var(--ely-divider)'}}>
      <div style={{flex:1}}>
        <div style={{fontSize:13, fontWeight:500}}>{title}</div>
        {desc && <div style={{fontSize:11.5, color:'var(--ely-ink-3)', marginTop:2}}>{desc}</div>}
      </div>
      {children}
    </div>
  );
  const Select = ({ value }) => (
    <span style={{display:'inline-flex',alignItems:'center', gap:6, padding:'5px 10px', background:'rgba(255,255,255,0.85)', borderRadius:6, boxShadow:'var(--ely-shadow-soft)', fontSize:12}}>{value} <I.ChevronDown size={11}/></span>
  );
  return (
  <ElyShell wallpaper="dawn" sidebarProps={{ activeId:'settings' }}>
    <div className="ely-topbar">
      <button className="ely-iconbtn"><I.ArrowLeft size={16}/></button>
      <button className="ely-iconbtn disabled"><I.ArrowRight size={16}/></button>
      <div className="ely-omnibar"><I.Settings size={14}/><span className="url-host">ely://settings</span><span className="url-path">/appearance</span><span className="right"><I.Filters size={14}/></span></div>
      <button className="ely-iconbtn"><I.Copy size={16}/></button>
      <button className="ely-iconbtn"><I.Download size={16}/></button>
      <button className="ely-iconbtn"><I.Moon size={16}/></button>
      <button className="ely-iconbtn"><I.Menu size={16}/></button>
    </div>
    <div style={{flex:1, display:'grid', gridTemplateColumns:'232px 1fr', minHeight:0}}>
      <div style={{padding:'16px 10px', borderRight:'1px solid var(--ely-divider)', overflowY:'auto'}}>
        <div style={{padding:'4px 12px 12px'}}>
          <div style={{fontSize:18, fontWeight:500}} className="ely-serif">Settings</div>
          <div style={{fontSize:11.5, color:'var(--ely-ink-4)', marginTop:2}}>ELY 0.42 · macOS 14.4</div>
        </div>
        <SectionLabel>General</SectionLabel>
        <NavItem icon={<I.Palette size={14}/>} label="Appearance" active/>
        <NavItem icon={<I.Sidebar size={14}/>} label="Sidebar"/>
        <NavItem icon={<I.Tab size={14}/>} label="Tabs &amp; Workspaces"/>
        <NavItem icon={<I.Cmd size={14}/>} label="Shortcuts"/>
        <SectionLabel>Account</SectionLabel>
        <NavItem icon={<I.Cloud size={14}/>} label="Sync"/>
        <NavItem icon={<I.Shield size={14}/>} label="Privacy &amp; Security"/>
        <NavItem icon={<I.User size={14}/>} label="Profiles"/>
        <SectionLabel>Power</SectionLabel>
        <NavItem icon={<I.Plug size={14}/>} label="Plugins"/>
        <NavItem icon={<I.Sparkle size={14}/>} label="ELY AI"/>
        <NavItem icon={<I.Code size={14}/>} label="Developer"/>
        <SectionLabel>About</SectionLabel>
        <NavItem icon={<I.Heart size={14}/>} label="What's new"/>
        <NavItem icon={<I.Bell size={14}/>} label="Help &amp; feedback"/>
      </div>

      <div className="ely-scroll" style={{padding:'28px 40px 32px', overflowY:'auto'}}>
        <div style={{maxWidth:780, margin:'0 auto'}}>
          <div style={{fontSize:11, color:'var(--ely-ink-4)', textTransform:'uppercase', letterSpacing:'0.1em'}}>General</div>
          <h1 className="ely-serif" style={{fontSize:34, fontWeight:400, margin:'4px 0 6px', letterSpacing:'-0.02em'}}>Appearance</h1>
          <div style={{fontSize:13, color:'var(--ely-ink-3)', maxWidth:520}}>Tune the atmosphere of your browser. ELY's theme follows the time of day by default — pick a wallpaper, then let it breathe.</div>

          <div style={{marginTop:24, display:'grid', gridTemplateColumns:'repeat(4, 1fr)', gap:10}}>
            {[
              ['Dawn','#ffd1d8','#b9c7ff', true],
              ['Violet','#d6c4ff','#ffe0c0'],
              ['Mint','#c5f0db','#ffe5b8'],
              ['Slate','#cfd6e2','#1d1c1a']
            ].map(([n, a, b, sel])=>(
              <div key={n} style={{position:'relative'}}>
                <div style={{aspectRatio:'4/3', borderRadius:10, background:`radial-gradient(70% 60% at 80% 20%, ${a}, transparent 70%), radial-gradient(60% 60% at 15% 90%, ${b}, transparent 70%), #f6f4ef`, boxShadow:'var(--ely-shadow-card)', outline: sel?'2px solid var(--ely-accent)':'none', outlineOffset:2}}/>
                <div style={{display:'flex', alignItems:'center', gap:6, padding:'8px 2px 0', fontSize:12}}>
                  <span style={{flex:1, fontWeight:500}}>{n}</span>
                  {sel && <span style={{color:'var(--ely-accent)'}}><I.Check size={13}/></span>}
                </div>
              </div>
            ))}
          </div>

          <div style={{marginTop:32}}>
            <Row title="Theme mode" desc="Match system appearance, or pick one to lock.">
              <div style={{display:'flex', gap:4, padding:3, background:'rgba(40,30,20,0.05)', borderRadius:8}}>
                {['System','Light','Dark'].map((t,i)=>(
                  <span key={t} style={{padding:'4px 12px', borderRadius:6, fontSize:12, background: i===0?'#fff':'transparent', boxShadow: i===0?'var(--ely-shadow-soft)':'none', fontWeight:500}}>{t}</span>
                ))}
              </div>
            </Row>
            <Row title="Accent color" desc="Used across selection, focus rings & links.">
              <div style={{display:'flex', gap:8}}>
                {['#c96442','#ec6a8e','#7c6cf7','#4fb59a','#1d1c1a'].map((c,i)=>(
                  <div key={c} style={{width:22,height:22, borderRadius:'50%', background:c, boxShadow: i===0?'0 0 0 2px #fff, 0 0 0 4px var(--ely-accent)':'inset 0 0 0 1px rgba(0,0,0,0.05)'}}/>
                ))}
              </div>
            </Row>
            <Row title="Font" desc="Display serif used for headlines & New Tab.">
              <Select value="Newsreader"/>
            </Row>
            <Row title="Translucency" desc="Glass density of sidebar & panels.">
              <div style={{width:200, height:4, borderRadius:2, background:'rgba(40,30,20,0.1)', position:'relative'}}>
                <div style={{position:'absolute', left:0, top:0, bottom:0, width:'62%', background:'var(--ely-accent)', borderRadius:2}}/>
                <div style={{position:'absolute', left:'62%', top:'50%', width:14, height:14, borderRadius:'50%', background:'#fff', boxShadow:'0 1px 4px rgba(0,0,0,0.2)', transform:'translate(-50%,-50%)'}}/>
              </div>
            </Row>
            <Row title="Reduce motion" desc="Mute transitions & ambient effects.">
              <Toggle/>
            </Row>
            <Row title="Reading-mode background" last>
              <Select value="Sepia"/>
            </Row>
          </div>

          <div style={{marginTop:36, fontSize:11, color:'var(--ely-ink-4)', textTransform:'uppercase', letterSpacing:'0.1em'}}>Sidebar</div>
          <h2 className="ely-serif" style={{fontSize:22, fontWeight:400, margin:'2px 0 18px'}}>Layout</h2>
          <div style={{display:'grid', gridTemplateColumns:'repeat(3, 1fr)', gap:10}}>
            {['Single column','Compact','Hidden on hover'].map((n,i)=>(
              <div key={n} className="ely-card" style={{padding:14, outline: i===0?'2px solid var(--ely-accent)':'none'}}>
                <div style={{aspectRatio:'5/3', background:'#fff', borderRadius:8, boxShadow:'var(--ely-shadow-soft)', position:'relative', overflow:'hidden'}}>
                  <div style={{position:'absolute', left:6, top:6, bottom:6, width: i===2?6:i===1?32:54, background:'rgba(40,30,20,0.08)', borderRadius:5}}/>
                  <div style={{position:'absolute', left: i===2?20:i===1?46:68, top:6, right:6, bottom:6, background:'rgba(40,30,20,0.04)', borderRadius:5}}/>
                </div>
                <div style={{fontSize:12, fontWeight:500, marginTop:10}}>{n}</div>
                <div style={{fontSize:11, color:'var(--ely-ink-4)', marginTop:2}}>{['Default · workspace + tabs','Icons-only with tooltips','Slide in on cursor reach'][i]}</div>
              </div>
            ))}
          </div>

        </div>
      </div>
    </div>
  </ElyShell>
);};
window.SettingsScreen = SettingsScreen;
