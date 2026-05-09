// split.jsx — Split-view browsing
const SplitScreen = () => {
  const Pane = ({ title, host, children, accent }) => (
    <div style={{display:'flex', flexDirection:'column', flex:1, minWidth:0, background:'#fff', borderRadius:10, overflow:'hidden', boxShadow:'0 1px 0 rgba(0,0,0,0.05), 0 12px 30px -16px rgba(0,0,0,0.18)'}}>
      <div style={{height:30, background:'#f6f4ef', borderBottom:'1px solid #ebe7e0', display:'flex', alignItems:'center', padding:'0 10px', gap:8, fontSize:11, color:'#3a3733'}}>
        <span style={{width:8, height:8, borderRadius:'50%', background:accent}}/>
        <I.Lock size={11}/>
        <span style={{color:'#1d1c1a', fontWeight:500}}>{host}</span>
        <span style={{color:'#9a948c'}}>{title}</span>
        <span style={{marginLeft:'auto', display:'flex', gap:6, color:'#9a948c'}}>
          <I.Reload size={11}/><I.X size={11}/>
        </span>
      </div>
      <div style={{flex:1, minHeight:0, overflow:'hidden'}}>{children}</div>
    </div>
  );
  return (
  <ElyShell wallpaper="violet" sidebarProps={{ activeId:'figma', mode:'browse', tabs:[
    { id:'figma', label:'Style Guide · Figma', icon:<Brand.Figma s={16}/>, active:true, closable:true },
    { id:'lin', label:'Roadmap 2024 — Linear', icon:<Brand.Linear s={16}/>, active:true },
    { id:'gh',  label:'elybrowser/core', icon:<Brand.GitHub s={16}/> },
  ] }}>
    <div className="ely-topbar" style={{gap:6}}>
      <button className="ely-iconbtn"><I.ArrowLeft size={16}/></button>
      <button className="ely-iconbtn disabled"><I.ArrowRight size={16}/></button>
      <div className="ely-omnibar" style={{flex:1}}>
        <I.Lock size={14}/>
        <span className="url-host">figma.com</span>
        <span className="url-path">/file/12345/Style-Guide</span>
        <span className="right"><I.Filters size={14}/><I.Star size={14}/></span>
      </div>
      <div style={{width:1, height:22, background:'var(--ely-divider)', margin:'0 4px'}}/>
      <div className="ely-omnibar" style={{flex:1}}>
        <I.Lock size={14}/>
        <span className="url-host">linear.app</span>
        <span className="url-path">/elybrowser/roadmap</span>
        <span className="right"><I.Filters size={14}/><I.Star size={14}/></span>
      </div>
      <button className="ely-iconbtn is-on" title="Split"><I.Split size={16}/></button>
      <button className="ely-iconbtn"><I.Menu size={16}/></button>
    </div>
    <div style={{flex:1, padding:14, display:'flex', gap:10, minHeight:0}}>
      {/* Left: Figma */}
      <Pane title="· Style Guide" host="figma.com" accent="#7c6cf7">
        <div style={{height:'100%', display:'grid', gridTemplateRows:'auto 1fr', background:'#1d1c1a', color:'#cfcbc4'}}>
          <div style={{padding:'8px 12px', fontSize:11, display:'flex', gap:8, alignItems:'center'}}>
            <Brand.Figma s={14}/>
            <span style={{color:'#fff', fontWeight:500}}>Style Guide</span>
            <span style={{opacity:.6}}>/ Typography</span>
            <span style={{marginLeft:'auto', display:'flex', gap:6}}>
              <span style={{padding:'2px 8px', background:'#fff', color:'#1d1c1a', borderRadius:4, fontSize:10.5}}>Share</span>
            </span>
          </div>
          <div style={{position:'relative', overflow:'hidden', background:'#f5f4f1'}}>
            <div style={{position:'absolute', inset:0, backgroundImage:'radial-gradient(circle at 1px 1px, rgba(0,0,0,0.05) 1px, transparent 0)', backgroundSize:'16px 16px'}}/>
            <div style={{position:'relative', margin:'24px auto', maxWidth:380, background:'#fff', padding:'24px 28px', borderRadius:6, boxShadow:'0 1px 0 rgba(0,0,0,0.06)'}}>
              <div className="ely-serif" style={{fontSize:36, lineHeight:1.05}}>The aesthetics of calm tech.</div>
              <div style={{marginTop:14, fontSize:11.5, color:'#3a3733', lineHeight:1.5}}>A type system tuned for long-form reading and slow, deliberate interfaces. Designed for ambient use.</div>
              <div style={{marginTop:14, display:'flex', gap:6}}>
                {['#1d1c1a','#c96442','#ec6a8e','#7c6cf7','#4fb59a'].map(c=>(<div key={c} style={{width:18,height:18,borderRadius:4, background:c}}/>))}
              </div>
            </div>
          </div>
        </div>
      </Pane>
      {/* Right: Linear */}
      <Pane title="· Roadmap 2024" host="linear.app" accent="#5e6ad2">
        <div style={{height:'100%', background:'#0e0d0c', color:'#dcd9d2', display:'flex'}}>
          <div style={{width:140, borderRight:'1px solid #1d1c1a', padding:'10px 8px', fontSize:11}}>
            <div style={{display:'flex',alignItems:'center',gap:6, padding:'4px 6px'}}><Brand.Linear s={14}/><span style={{fontWeight:600}}>ELY</span></div>
            {['Inbox','My Issues','Triage'].map(l=>(<div key={l} style={{padding:'4px 6px', color:'#9a948c'}}>{l}</div>))}
            <div style={{padding:'8px 6px 2px', color:'#6b6660', fontSize:10, textTransform:'uppercase', letterSpacing:'.08em'}}>Cycles</div>
            {['24 · Now','24 · Next','25 · Q1','25 · Q2'].map((l,i)=>(<div key={l} style={{padding:'4px 6px', borderRadius:4, background:i===0?'#1d1c1a':'transparent', color:i===0?'#fff':'#9a948c'}}>{l}</div>))}
          </div>
          <div style={{flex:1, padding:'10px 14px', fontSize:11.5}}>
            <div style={{display:'flex', alignItems:'center', gap:8}}>
              <span style={{fontSize:13, color:'#fff', fontWeight:500}}>Roadmap 2024</span>
              <span style={{color:'#6b6660'}}>·  18 issues</span>
              <span style={{marginLeft:'auto', padding:'2px 8px', background:'#5e6ad2', color:'#fff', borderRadius:4, fontSize:10.5}}>+ New</span>
            </div>
            <div style={{display:'grid', gridTemplateColumns:'1fr', gap:1, marginTop:10, background:'#1d1c1a', borderRadius:4, overflow:'hidden'}}>
              {[
                ['ELY-101','In Progress', 'Universal command bar', '#f5a623'],
                ['ELY-102','In Progress', 'Vertical tabs · sticky pinning', '#f5a623'],
                ['ELY-103','Todo', 'Cloudflare sync · device pairing', '#5e6ad2'],
                ['ELY-104','Todo', 'Plugin SDK · v0', '#5e6ad2'],
                ['ELY-105','Backlog', 'Reader mode · ambient theme', '#6b6660'],
                ['ELY-106','Done', 'Workspace switching', '#4fb59a'],
              ].map(([id,st,t,c],i)=>(
                <div key={i} style={{display:'flex',alignItems:'center', gap:10, padding:'8px 10px', background:'#0e0d0c'}}>
                  <span style={{width:8,height:8,borderRadius:'50%', background:c}}/>
                  <span style={{color:'#6b6660', width:54}}>{id}</span>
                  <span style={{flex:1, color:'#fff'}}>{t}</span>
                  <span style={{color:'#9a948c'}}>{st}</span>
                  <span style={{width:18,height:18,borderRadius:'50%', background:'linear-gradient(135deg,#7c6cf7,#ec6a8e)'}}/>
                </div>
              ))}
            </div>
          </div>
        </div>
      </Pane>
    </div>
  </ElyShell>
);};
window.SplitScreen = SplitScreen;
