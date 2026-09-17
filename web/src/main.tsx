import React, {useCallback, useEffect, useMemo, useState} from 'react';
import {createRoot} from 'react-dom/client';
import {ArrowDownToLine, ArrowRight, Box, Check, Copy, Database, Flower2, Fuel, Pickaxe, Search, Send, WalletCards} from 'lucide-react';
import './styles.css';

type Pool = 'transparent' | 'orchard';
type Account = {id:number; name:string; unified_address:string; transparent_address:string; transparent_zatoshi:number; orchard_zatoshi:number};
type Activity = {id:string; kind:string; from_account?:number; to_account:number; source_pool:Pool; destination_pool:Pool; amount_zatoshi:number; txid:string; block_hash?:string; status:string; created_at:string};
type Status = {instance:string; network:string; auto_mine:boolean; endpoints:{dashboard:string;zakura_rpc:string;lightwalletd:string;p2p:string}; node?:{blocks:number; bestblockhash:string; verificationprogress:number}};

const api = async <T,>(path:string, init?:RequestInit):Promise<T> => {
  const response = await fetch(`/api/v1${path}`, {headers:{'content-type':'application/json'}, ...init});
  const data = await response.json();
  if (!response.ok) throw new Error(data.error?.message ?? `Request failed (${response.status})`);
  return data;
};
const zec = (z:number) => `${(z / 100_000_000).toLocaleString(undefined,{maximumFractionDigits:8})} ZEC`;
const short = (s:string, length=13) => s ? `${s.slice(0,length)}…${s.slice(-6)}` : '—';
const key = () => crypto.randomUUID();

function App(){
  const [tab,setTab]=useState<'wallet'|'explorer'|'network'>('wallet');
  const [accounts,setAccounts]=useState<Account[]>([]);
  const [activity,setActivity]=useState<Activity[]>([]);
  const [status,setStatus]=useState<Status|null>(null);
  const [dialog,setDialog]=useState<'send'|'faucet'|'mine'|null>(null);
  const [notice,setNotice]=useState<string>('');
  const [error,setError]=useState<string>('');
  const refresh=useCallback(async()=>{
    const [a,h,s]=await Promise.all([api<Account[]>('/accounts'),api<Activity[]>('/activity'),api<Status>('/status')]);
    setAccounts(a);setActivity(h);setStatus(s);
  },[]);
  useEffect(()=>{ refresh().catch(e=>setError(e.message)); const events=new EventSource('/api/v1/events'); events.addEventListener('update',()=>refresh()); return()=>events.close();},[refresh]);
  const total=useMemo(()=>accounts.reduce((n,a)=>n+a.transparent_zatoshi+a.orchard_zatoshi,0),[accounts]);
  const done=(message:string)=>{setDialog(null);setNotice(message);setError('');refresh();setTimeout(()=>setNotice(''),3500)};
  return <div className="shell">
    <aside>
      <div className="brand"><div className="mark"><Flower2/></div><div><b>Thus Spoke</b><span>Zakura</span></div></div>
      <nav>
        <button className={tab==='wallet'?'active':''} onClick={()=>setTab('wallet')}><WalletCards/>Wallet</button>
        <button className={tab==='explorer'?'active':''} onClick={()=>setTab('explorer')}><Box/>Explorer</button>
        <button className={tab==='network'?'active':''} onClick={()=>setTab('network')}><Database/>Network</button>
      </nav>
      <div className="network-pill"><i></i><div><b>{status?.network ?? 'Connecting'}</b><span>Height {status?.node?.blocks ?? '—'}</span></div></div>
    </aside>
    <main>
      <header><div><p>INSTANCE / {status?.instance?.toUpperCase() ?? 'DEFAULT'}</p><h1>{tab[0].toUpperCase()+tab.slice(1)}</h1></div><div className="actions"><button className="ghost" onClick={()=>setDialog('mine')}><Pickaxe/>Mine</button><button className="primary" onClick={()=>setDialog('faucet')}><Fuel/>Faucet</button></div></header>
      {notice&&<div className="toast success"><Check/>{notice}</div>}{error&&<div className="toast error">{error}</div>}
      {tab==='wallet'&&<Wallet accounts={accounts} activity={activity} total={total} onSend={()=>setDialog('send')} onFaucet={()=>setDialog('faucet')}/>} 
      {tab==='explorer'&&<Explorer status={status} activity={activity}/>} 
      {tab==='network'&&<Network status={status}/>} 
    </main>
    {dialog&&<Dialog kind={dialog} accounts={accounts} close={()=>setDialog(null)} done={done} fail={(e)=>setError(e)}/>} 
  </div>
}

function Wallet({accounts,activity,total,onSend,onFaucet}:{accounts:Account[];activity:Activity[];total:number;onSend:()=>void;onFaucet:()=>void}){
 return <>
  <section className="hero"><div><span>Total balance</span><strong>{zec(total)}</strong><small>Across 5 deterministic development accounts</small></div><Flower2/></section>
  <div className="section-title"><div><h2>Accounts</h2><p>ZIP-32 derived · Regtest only</p></div><button className="ghost" onClick={onSend}><Send/>Send ZEC</button></div>
  <section className="account-grid">{accounts.map(a=><article className="account" key={a.id}>
   <div className="account-head"><div className={`avatar a${a.id}`}>{a.id}</div><div><h3>{a.name}</h3><span>{a.id===1?'Faucet treasury':'Development wallet'}</span></div><button aria-label="Copy unified address" onClick={()=>navigator.clipboard.writeText(a.unified_address)}><Copy/></button></div>
   <div className="balance-row"><div><span>Orchard</span><b>{zec(a.orchard_zatoshi)}</b></div><div><span>Transparent</span><b>{zec(a.transparent_zatoshi)}</b></div></div>
   <code title={a.unified_address}>{short(a.unified_address,17)}</code>
   <div className="account-actions"><button onClick={onFaucet}><ArrowDownToLine/>Fund</button><button onClick={onSend}>Send<ArrowRight/></button></div>
  </article>)}</section>
  <ActivityList activity={activity}/>
 </>
}

function ActivityList({activity}:{activity:Activity[]}){return <section className="panel"><div className="panel-title"><h2>Recent activity</h2><span>{activity.length} events</span></div>{activity.length===0?<div className="empty">Use the faucet to create your first Regtest activity.</div>:<div className="activity">{activity.map(a=><div className="activity-row" key={a.id}><div className={`tx-icon ${a.kind}`}><Send/></div><div><b>{a.kind==='faucet'?'Faucet deposit':`Account ${a.from_account} → Account ${a.to_account}`}</b><span>{a.source_pool} → {a.destination_pool}</span></div><code>{short(a.txid,9)}</code><strong>{zec(a.amount_zatoshi)}</strong><em className={a.status}>{a.status}</em></div>)}</div>}</section>}

function Explorer({status,activity}:{status:Status|null;activity:Activity[]}){
 const [query,setQuery]=useState(''); const [result,setResult]=useState<any>(null); const [error,setError]=useState('');
 const search=async(e:React.FormEvent)=>{e.preventDefault();setError('');try{setResult(await api(`/search?q=${encodeURIComponent(query)}`))}catch(e){setError((e as Error).message)}};
 return <><form className="search" onSubmit={search}><Search/><input value={query} onChange={e=>setQuery(e.target.value)} placeholder="Search block height, hash, or transaction ID"/><button>Search</button></form>
 <section className="stats"><article><span>Chain height</span><strong>{status?.node?.blocks ?? '—'}</strong></article><article><span>Wallet transactions</span><strong>{activity.length}</strong></article><article><span>Network</span><strong>Regtest</strong></article></section>
 {error&&<div className="toast error">{error}</div>}{result&&<section className="panel raw"><div className="panel-title"><h2>RPC result</h2><span>Raw JSON</span></div><pre>{JSON.stringify(result,null,2)}</pre></section>}
 <div className="privacy-note"><Flower2/><div><b>Shielded by design</b><p>Orchard recipients and note values are not public chain data. Owned details appear in the wallet because it holds the viewing keys.</p></div></div><ActivityList activity={activity}/></>;
}

function Network({status}:{status:Status|null}){return <><section className="hero compact"><div><span>Node health</span><strong>{status?.node?'All systems operational':'Waiting for Zakura'}</strong><small>Auto-mining is {status?.auto_mine?'enabled':'disabled'}</small></div><i className={status?.node?'online':''}></i></section><section className="panel details"><div><span>Network</span><b>{status?.network}</b></div><div><span>Current height</span><b>{status?.node?.blocks ?? '—'}</b></div><div><span>Best block</span><code>{short(status?.node?.bestblockhash??'',18)}</code></div><div><span>Verification</span><b>{Math.round((status?.node?.verificationprogress??0)*100)}%</b></div></section><section className="panel"><div className="panel-title"><h2>Runtime endpoints</h2><span>Host network</span></div><div className="details endpoints"><div><span>Dashboard</span><code>{status?.endpoints.dashboard ?? '—'}</code></div><div><span>Zakura RPC</span><code>{status?.endpoints.zakura_rpc ?? '—'}</code></div><div><span>lightwalletd</span><code>{status?.endpoints.lightwalletd ?? '—'}</code></div><div><span>P2P</span><code>{status?.endpoints.p2p ?? '—'}</code></div></div></section></>}

function Dialog({kind,accounts,close,done,fail}:{kind:'send'|'faucet'|'mine';accounts:Account[];close:()=>void;done:(s:string)=>void;fail:(s:string)=>void}){
 const [from,setFrom]=useState(1),[to,setTo]=useState(kind==='send'?2:1),[source,setSource]=useState<Pool>('orchard'),[dest,setDest]=useState<Pool>('orchard'),[amount,setAmount]=useState(kind==='faucet'?'5':'10'),[blocks,setBlocks]=useState('1'),[busy,setBusy]=useState(false);
 const faucetAmount=Number(amount),faucetAmountError=kind!=='faucet'?'':amount.trim()===''?'Enter an amount greater than 0 ZEC.':!Number.isFinite(faucetAmount)||faucetAmount<=0?'Enter an amount greater than 0 ZEC.':faucetAmount>5?'Enter no more than 5 ZEC per faucet request.':'';
 const submit=async(e:React.FormEvent)=>{e.preventDefault();if(faucetAmountError)return;setBusy(true);try{if(kind==='mine'){const blockCount=Number(blocks);if(!Number.isInteger(blockCount)||blockCount<1||blockCount>10000)throw new Error('Number of blocks must be an integer between 1 and 10,000');await api('/mine',{method:'POST',body:JSON.stringify({blocks:blockCount})});done(`Mined ${blockCount} block${blockCount===1?'':'s'}`)}else{const amount_zatoshi=Math.round(Number(amount)*100_000_000);if(kind==='faucet'){await api('/faucet',{method:'POST',body:JSON.stringify({account_id:to,pool:dest,amount_zatoshi,idempotency_key:key()})});done(`Funded Account ${to}`)}else{await api('/send',{method:'POST',body:JSON.stringify({from_account:from,to_account:to,source_pool:source,destination_pool:dest,amount_zatoshi,idempotency_key:key()})});done(`Sent to Account ${to}`)}}}catch(e){fail((e as Error).message)}finally{setBusy(false)}};
 return <div className="overlay" onMouseDown={e=>e.target===e.currentTarget&&close()}><form className="dialog" onSubmit={submit}><div className="dialog-head"><div><span>{kind==='faucet'?'DEV TOOLS':kind==='mine'?'CHAIN CONTROL':'NEW TRANSACTION'}</span><h2>{kind==='faucet'?'Fund an account':kind==='mine'?'Mine blocks':'Send ZEC'}</h2></div><button type="button" onClick={close}>×</button></div>
 {kind==='send'&&<div className="form-grid"><label>From account<select value={from} onChange={e=>setFrom(+e.target.value)}>{accounts.map(a=><option value={a.id} key={a.id}>{a.name}</option>)}</select></label><label>Source pool<select value={source} onChange={e=>setSource(e.target.value as Pool)}><option value="orchard">Orchard</option><option value="transparent">Transparent</option></select></label></div>}
 {kind!=='mine'&&<><label>Destination account<select value={to} onChange={e=>setTo(+e.target.value)}>{accounts.map(a=><option value={a.id} key={a.id}>{a.name}</option>)}</select></label><label>Destination pool<select value={dest} onChange={e=>setDest(e.target.value as Pool)}><option value="orchard">Orchard shielded</option><option value="transparent">Transparent</option></select></label><label>Amount (ZEC)<input type="number" min="0.00000001" max={kind==='faucet'?'5':undefined} step="0.00000001" required value={amount} aria-invalid={Boolean(faucetAmountError)} aria-describedby={kind==='faucet'?'faucet-amount-help':undefined} onChange={e=>setAmount(e.target.value)}/>{kind==='faucet'&&<span id="faucet-amount-help" role={faucetAmountError?'alert':undefined} style={{color:faucetAmountError?'#f4a8a8':'var(--muted)'}}>{faucetAmountError||'Maximum 5 ZEC per faucet request.'}</span>}</label></>}
 {kind==='mine'&&<label>Number of blocks<input type="number" min="1" max="10000" required value={blocks} onChange={e=>setBlocks(e.target.value)}/></label>}
 <div className="auto"><Pickaxe/><div><b>Auto-confirm</b><span>{kind==='mine'?'Blocks use Regtest instant mining.':'One block will be mined after this action.'}</span></div></div><button className="primary submit" disabled={busy||Boolean(faucetAmountError)}>{busy?'Working…':kind==='mine'?'Mine blocks':kind==='faucet'?'Add funds':'Review & send'}</button></form></div>
}

createRoot(document.getElementById('root')!).render(<React.StrictMode><App/></React.StrictMode>);
