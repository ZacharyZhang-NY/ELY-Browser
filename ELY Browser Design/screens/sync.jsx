// sync.jsx — Sync / Sign In page
const SyncScreen = () => {
  const Device = ({ name, model, when, current, icon }) => (
    <div style={{display:'flex',alignItems:'center', gap:12, padding:'14px 4px', borderBottom:'1px solid var(--ely-divider)'}}>
      <div style={{width:36, height:36, borderRadius:9, background:'rgba(255,255,255,0.85)', boxShadow:'var(--ely-shadow-soft)', display:'flex',alignItems:'center',justifyContent:'center', color:'var(--ely-ink-2)'}}>{icon}</div>
      <div style={{flex:1}}>
        <div style={{fontSize:13, fontWeight:500, display:'flex', alignItems:'center', gap:8}}>{name}{current && <span style={{padding:'1px 7px', background:'rgba(79,181,154,0.18)', color:'#2f7d68', borderRadius:999, fontSize:10}}>This device</span>}</div>
        <div style={{fontSize:11, color:'var(--ely-ink-4)', marginTop:1}}>{model} · {when}</div>
      </div>
      <span style={{fontSize:11, color:'var(--ely-ink-3)', display:'inline-flex',alignItems:'center', gap:4}}><span style={{width:6, height:6, borderRadius:'50%', background:'#4fb59a'}}/> Synced</span>
    </div>
  );
  return (
  <ElyShell wallpaper="violet" sidebarProps={{ activeId:'settings' }}>
    <div className="ely-topbar">
      <button className="ely-iconbtn"><I.ArrowLeft size={16}/></button>
      <button className="ely-iconbtn disabled"><I.ArrowRight size={16}/></button>
      <div className="ely-omnibar"><I.Cloud size={14}/><span className="url-host">ely://settings</span><span className="url-path">/sync</span></div>
      <button className="ely-iconbtn"><I.Moon size={16}/></button>
      <button className="ely-iconbtn"><I.Menu size={16}/></button>
    </div>
    <div className="ely-scroll" style={{flex:1, overflowY:'auto', padding:'40px 56px 32px'}}>
      <div style={{maxWidth:880, margin:'0 auto', display:'grid', gridTemplateColumns:'1.1fr 1fr', gap:32, alignItems:'start'}}>
        <div>
          <div style={{display:'inline-flex', alignItems:'center', gap:8, padding:'4px 10px', borderRadius:999, background:'rgba(255,255,255,0.7)', boxShadow:'var(--ely-shadow-soft)', fontSize:11, color:'var(--ely-ink-3)'}}>
            <I.Cloud size={12}/> Cloudflare Sync · end-to-end encrypted
          </div>
          <h1 className="ely-serif" style={{fontSize:46, lineHeight:1.05, margin:'16px 0 10px', fontWeight:400, letterSpacing:'-0.02em'}}>Your tabs, on every device.</h1>
          <p style={{fontSize:14, color:'var(--ely-ink-2)', lineHeight:1.55, maxWidth:440, margin:0}}>Sign in once and ELY keeps your tabs, workspaces, pinned items and history in sync — encrypted in your hands, mirrored at the edge.</p>

          <div style={{marginTop:28, padding:24, borderRadius:16, background:'rgba(255,255,255,0.85)', boxShadow:'var(--ely-shadow-card)', maxWidth:380}}>
            <div style={{fontSize:12, color:'var(--ely-ink-3)', marginBottom:14, display:'flex', alignItems:'center', gap:6}}>
              <I.Lock size={12}/> Better Auth · ELY Account
            </div>
            <label style={{display:'block', fontSize:11, color:'var(--ely-ink-3)', marginBottom:6}}>Email</label>
            <div style={{height:40, padding:'0 12px', borderRadius:9, background:'#fff', boxShadow:'var(--ely-shadow-soft)', display:'flex', alignItems:'center', fontSize:13, color:'var(--ely-ink)'}}>alex@elybrowser.com</div>

            <label style={{display:'block', fontSize:11, color:'var(--ely-ink-3)', marginBottom:6, marginTop:14}}>Password</label>
            <div style={{height:40, padding:'0 12px', borderRadius:9, background:'#fff', boxShadow:'var(--ely-shadow-soft)', display:'flex', alignItems:'center', fontSize:13, color:'var(--ely-ink)', letterSpacing:'0.2em'}}>••••••••••</div>

            <button style={{width:'100%', marginTop:18, height:42, borderRadius:10, background:'var(--ely-ink)', color:'#fff', fontWeight:500, fontSize:13, border:0, display:'flex', alignItems:'center', justifyContent:'center', gap:8}}>
              Sign in <I.ArrowRight size={14}/>
            </button>
            <div style={{display:'flex', alignItems:'center', gap:10, margin:'14px 0 12px'}}>
              <div style={{flex:1, height:1, background:'var(--ely-divider)'}}/><span style={{fontSize:10.5, color:'var(--ely-ink-4)'}}>or continue with</span><div style={{flex:1, height:1, background:'var(--ely-divider)'}}/>
            </div>
            <div style={{display:'grid', gridTemplateColumns:'1fr 1fr 1fr', gap:8}}>
              {['Google','GitHub','Apple'].map(p=>(
                <div key={p} style={{height:36, borderRadius:9, background:'#fff', boxShadow:'var(--ely-shadow-soft)', display:'flex', alignItems:'center', justifyContent:'center', gap:6, fontSize:12, fontWeight:500, color:'var(--ely-ink)'}}>
                  {p==='GitHub' && <Brand.GitHub s={13}/>}
                  {p==='Google' && <span style={{width:13, height:13, borderRadius:'50%', background:'conic-gradient(from 90deg, #ea4335, #fbbc05, #34a853, #4285f4)'}}/>}
                  {p==='Apple' && <span style={{fontSize:13}}></span>}
                  {p}
                </div>
              ))}
            </div>
            <div style={{marginTop:14, fontSize:11, color:'var(--ely-ink-4)', textAlign:'center'}}>By signing in you accept the ELY <span style={{color:'var(--ely-ink-2)', textDecoration:'underline', textDecorationColor:'var(--ely-ink-5)'}}>terms</span> &amp; <span style={{color:'var(--ely-ink-2)', textDecoration:'underline', textDecorationColor:'var(--ely-ink-5)'}}>privacy</span>.</div>
          </div>
        </div>

        <div style={{display:'flex', flexDirection:'column', gap:14}}>
          <div className="ely-card" style={{padding:18}}>
            <div style={{display:'flex', alignItems:'center', gap:10, marginBottom:6}}>
              <span style={{width:28, height:28, borderRadius:8, background:'linear-gradient(135deg,#fff,#f0e9e2)', display:'flex',alignItems:'center',justifyContent:'center', boxShadow:'var(--ely-shadow-soft)', color:'var(--ely-accent)'}}><I.Devices size={14}/></span>
              <div style={{fontSize:13, fontWeight:500, flex:1}}>Devices</div>
              <span style={{fontSize:11, color:'var(--ely-ink-3)'}}>3 connected</span>
            </div>
            <Device current name="Alex's MacBook Pro" model="macOS 14.4 · Apple Silicon" when="Active now" icon={<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><rect x="3" y="4" width="18" height="12" rx="1.5"/><path d="M2 20h20"/></svg>}/>
            <Device name="iPhone 15 Pro" model="iOS 17.4 · Cellular" when="2 minutes ago" icon={<svg width="14" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><rect x="6" y="2" width="12" height="20" rx="2"/><path d="M11 18h2"/></svg>}/>
            <Device name="Studio iMac" model="macOS 14.2 · Office" when="6 hours ago" icon={<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><rect x="2" y="3" width="20" height="14" rx="1.5"/><path d="M8 21h8M12 17v4"/></svg>}/>
            <div style={{padding:'12px 4px 0', display:'flex', alignItems:'center', gap:10}}>
              <span style={{padding:'7px 14px', borderRadius:8, background:'#fff', boxShadow:'var(--ely-shadow-soft)', fontSize:12, fontWeight:500, display:'inline-flex', alignItems:'center', gap:6}}><I.Plus size={12}/> Pair a device</span>
              <span style={{fontSize:11, color:'var(--ely-ink-4)'}}>or scan QR with the ELY app</span>
            </div>
          </div>

          <div className="ely-card" style={{padding:18, background:'linear-gradient(135deg, rgba(255,235,225,0.95), rgba(232,224,255,0.85))'}}>
            <div style={{display:'flex', alignItems:'flex-start', gap:12}}>
              <span style={{width:30, height:30, borderRadius:8, background:'#fff', display:'flex',alignItems:'center',justifyContent:'center', boxShadow:'var(--ely-shadow-soft)', color:'var(--ely-accent)'}}><I.Key size={15}/></span>
              <div style={{flex:1}}>
                <div style={{fontSize:13, fontWeight:500}}>Recovery key</div>
                <div style={{fontSize:11.5, color:'var(--ely-ink-3)', marginTop:2}}>The only way to restore an end-to-end encrypted account. We don't keep a copy.</div>
              </div>
            </div>
            <div className="ely-mono" style={{marginTop:14, padding:14, background:'rgba(255,255,255,0.85)', borderRadius:10, fontSize:13, letterSpacing:'0.05em', lineHeight:1.7, color:'var(--ely-ink)', textAlign:'center', fontWeight:500, boxShadow:'var(--ely-shadow-soft)'}}>
              river · slate · candle · lantern<br/>orchid · meadow · whistle · ember
            </div>
            <div style={{display:'flex', gap:8, marginTop:12}}>
              <span style={{flex:1, padding:'8px 10px', borderRadius:8, background:'#fff', boxShadow:'var(--ely-shadow-soft)', fontSize:12, fontWeight:500, textAlign:'center'}}>Copy</span>
              <span style={{flex:1, padding:'8px 10px', borderRadius:8, background:'#fff', boxShadow:'var(--ely-shadow-soft)', fontSize:12, fontWeight:500, textAlign:'center'}}>Download .txt</span>
              <span style={{flex:1, padding:'8px 10px', borderRadius:8, background:'var(--ely-ink)', color:'#fff', fontSize:12, fontWeight:500, textAlign:'center'}}>I've saved it</span>
            </div>
          </div>

          <div className="ely-card" style={{padding:16}}>
            <div style={{fontSize:13, fontWeight:500, marginBottom:8}}>What syncs</div>
            <div style={{display:'grid', gridTemplateColumns:'1fr 1fr', gap:6, fontSize:12, color:'var(--ely-ink-2)'}}>
              {[['Tabs &amp; pinned',true],['Workspaces',true],['History',true],['Bookmarks',true],['Reading list',true],['Plugins',true],['Passwords',false],['Open windows',false]].map(([n,on],i)=>(
                <div key={i} style={{display:'flex',alignItems:'center',gap:8, padding:'4px 0'}}>
                  <span style={{color: on?'#2f7d68':'var(--ely-ink-4)'}}>{on? <I.Check size={13}/> : <I.X size={13}/>}</span>
                  <span dangerouslySetInnerHTML={{__html:n}}/>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  </ElyShell>
);};
window.SyncScreen = SyncScreen;
