// browsing.jsx — Main browsing page (real webpage open)
const BrowsingScreen = () => (
  <ElyShell wallpaper="dawn" sidebarProps={{ activeId:'figma', mode:'browse', tabs:[
    { id:'figma', label:'Style Guide · Figma', icon:<Brand.Figma s={16}/>, active:true, audio:true, closable:true },
    { id:'roadmap', label:'Roadmap 2024 — Linear', icon:<Brand.Linear s={16}/> },
    { id:'mkt', label:'Marketing Site', icon:<Icon size={16} stroke="#6b6660"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18"/></Icon> },
    { id:'twt', label:'Home / X', icon:<Brand.X s={16}/> },
    { id:'gh',  label:'elybrowser/core · main', icon:<Brand.GitHub s={16}/> },
  ] }}>
    <TopBar host="figma.com" path="/file/12345/Style-Guide" canBack canForward/>
    <div className="ely-scroll" style={{flex:1, overflowY:'auto', background:'#fff'}}>
      {/* Mock figma-ish UI inside */}
      <div style={{height:46, background:'#1d1c1a', color:'#cfcbc4', display:'flex', alignItems:'center', padding:'0 14px', fontSize:12, gap:10}}>
        <div style={{display:'flex',alignItems:'center',gap:6}}><Brand.Figma s={14}/><span style={{color:'#fff', fontWeight:500}}>Style Guide</span></div>
        <span style={{opacity:.55}}>/</span>
        <span>ELY Design System</span>
        <span style={{marginLeft:'auto', display:'flex', gap:8, alignItems:'center'}}>
          <span style={{padding:'2px 8px', background:'#fff', color:'#1d1c1a', borderRadius:4, fontSize:11, fontWeight:500}}>Share</span>
          <div style={{width:22,height:22,borderRadius:'50%',background:'linear-gradient(135deg,#7c6cf7,#ec6a8e)'}}/>
          <div style={{width:22,height:22,borderRadius:'50%',background:'linear-gradient(135deg,#4fb59a,#7c6cf7)'}}/>
        </span>
      </div>
      <div style={{display:'grid', gridTemplateColumns:'220px 1fr 260px', height:'calc(100% - 46px)', minHeight:560, background:'#f5f4f1'}}>
        <div style={{background:'#fff', borderRight:'1px solid #ebe7e0', padding:'10px 8px', fontSize:11.5, color:'#3a3733'}}>
          <div style={{padding:'4px 6px', fontWeight:600, color:'#1d1c1a'}}>Pages</div>
          {['Cover','Tokens','Typography','Color','Spacing','Components','Patterns','Sandbox'].map((p,i)=>(
            <div key={p} style={{padding:'5px 8px', borderRadius:5, background: i===2?'#f0e9e2':'transparent', color: i===2?'#1d1c1a':'#3a3733', display:'flex',alignItems:'center',gap:6}}>
              <I.ChevronRight size={10} style={{opacity:.6}}/> {p}
            </div>
          ))}
          <div style={{padding:'10px 6px 4px', fontWeight:600, color:'#1d1c1a', marginTop:8}}>Layers</div>
          {[['Frame · Display','◇'],['  H1 / Display','T'],['  H2 / Title','T'],['  H3 / Subtitle','T'],['Frame · Body','◇'],['  Paragraph','T'],['  Caption','T']].map(([n,g],i)=>(
            <div key={i} style={{padding:'4px 6px', fontFamily:'monospace', fontSize:10.5, color:'#6b6660', whiteSpace:'pre'}}>{g}  {n}</div>
          ))}
        </div>
        <div style={{padding:32, position:'relative', overflow:'hidden'}}>
          <div style={{position:'absolute', inset:0, backgroundImage:'radial-gradient(circle at 1px 1px, rgba(0,0,0,0.06) 1px, transparent 0)', backgroundSize:'18px 18px'}}/>
          <div style={{position:'relative', background:'#fff', borderRadius:6, padding:'32px 40px', boxShadow:'0 1px 0 rgba(0,0,0,0.06), 0 12px 40px -10px rgba(0,0,0,0.12)', maxWidth:520}}>
            <div style={{fontSize:10.5, color:'#9a948c', letterSpacing:'0.12em', textTransform:'uppercase'}}>Display / Serif</div>
            <div className="ely-serif" style={{fontSize:48, lineHeight:1.05, marginTop:8, color:'#1d1c1a'}}>The aesthetics<br/>of calm tech.</div>
            <div style={{marginTop:24, display:'grid', gridTemplateColumns:'1fr 1fr', gap:14}}>
              {['H1 / 64','H2 / 40','H3 / 28','Body / 16','Caption / 12','Mono / 13'].map((s,i)=>(
                <div key={i} style={{borderTop:'1px solid #ebe7e0', paddingTop:8}}>
                  <div className={i<3?'ely-serif':''} style={{fontSize: i<3? 18: 13, color:'#1d1c1a', fontFamily: i===5?'monospace':undefined}}>The quick brown fox</div>
                  <div style={{fontSize:10.5, color:'#9a948c', marginTop:2}}>{s}</div>
                </div>
              ))}
            </div>
          </div>
          <div style={{position:'absolute', bottom:14, left:14, display:'flex', gap:6, padding:'6px 10px', background:'#fff', borderRadius:6, boxShadow:'0 1px 4px rgba(0,0,0,0.08)', fontSize:11, color:'#3a3733'}}>
            100% <I.ChevronDown size={10}/>
          </div>
        </div>
        <div style={{background:'#fff', borderLeft:'1px solid #ebe7e0', padding:'12px 12px', fontSize:11.5, color:'#3a3733'}}>
          <div style={{display:'flex', gap:14, fontWeight:500, color:'#1d1c1a', borderBottom:'1px solid #ebe7e0', paddingBottom:8}}>
            <span style={{borderBottom:'1.5px solid #1d1c1a', paddingBottom:6}}>Design</span>
            <span style={{color:'#9a948c'}}>Prototype</span>
            <span style={{color:'#9a948c'}}>Inspect</span>
          </div>
          <div style={{paddingTop:10, color:'#9a948c'}}>Color · Local styles</div>
          <div style={{display:'grid', gridTemplateColumns:'repeat(6, 1fr)', gap:6, marginTop:8}}>
            {['#1d1c1a','#3a3733','#6b6660','#c96442','#ec6a8e','#7c6cf7','#4fb59a','#f6f4ef','#efe8e1','#fff3ee','#ffd1d8','#b9c7ff'].map(c=>(
              <div key={c} style={{aspectRatio:'1', borderRadius:4, background:c, boxShadow:'inset 0 0 0 1px rgba(0,0,0,0.05)'}}/>
            ))}
          </div>
          <div style={{paddingTop:14, color:'#9a948c'}}>Type · Newsreader / Geist</div>
          <div style={{padding:8, background:'#f6f4ef', borderRadius:6, marginTop:8, fontSize:11}}>
            <div className="ely-serif" style={{fontSize:18}}>Aa Bb Cc</div>
            <div style={{color:'#6b6660', marginTop:2}}>Newsreader · 6..72</div>
          </div>
          <div style={{paddingTop:14, color:'#9a948c'}}>Spacing</div>
          <div style={{display:'flex', gap:4, marginTop:6}}>
            {[4,8,12,16,24,32].map(s=>(<div key={s} style={{width:s, height:18, background:'#1d1c1a', opacity:.85}}/>))}
          </div>
        </div>
      </div>
    </div>
  </ElyShell>
);
window.BrowsingScreen = BrowsingScreen;
