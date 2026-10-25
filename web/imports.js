'use strict';
const ingestion = {items:[],active:null,page:1,regions:[],draft:false,busy:false,engines:[],selected:-1,stopRequested:false,aggregate:null,stage:null,started:0};
const importDialog = document.createElement('dialog');
importDialog.id='import-dialog';
importDialog.innerHTML=`<div class="dialog-heading"><div><p class="eyebrow">IMPORT / REVIEW / RECOGNIZE</p><h2>先分好题，再留下文字。</h2></div><button data-close="import-dialog" aria-label="关闭导入">×</button></div>
<p class="muted">多选 PDF、图片、Markdown 或 TXT。原件与每道题的原图区域会一起保留，方便日后对照。</p>
<section id="import-progress-panel" class="import-progress-panel" hidden aria-label="切分与识别进度"><div class="import-progress-heading"><strong id="import-progress-title">处理中</strong><output id="import-progress-count"></output></div><progress id="import-total-progress" max="1" aria-label="文件或题目完成进度"></progress><div class="import-stage-heading"><span id="import-progress-stage"></span><span id="import-progress-elapsed"></span></div><progress id="import-stage-progress" max="1" aria-label="当前处理阶段进度"></progress><div id="import-operation-status" role="status" aria-live="polite" hidden></div><details id="import-error-details" hidden><summary>查看技术详情</summary><pre></pre></details><div class="import-progress-actions"><button class="button" id="import-background">后台处理</button><button class="button" id="import-stop">停止后续任务</button></div></section>
<div class="import-controls"><div class="import-settings-summary"><span id="import-defaults"></span><button class="button icon-button" id="import-settings" title="修改导入设置" aria-label="修改导入设置">⚙</button></div>
<label class="checkline" id="import-remote-line" hidden><input type="checkbox" id="import-remote">同意将本次题目图片发送给所选外部 OCR 服务</label>
<label class="checkline"><input type="checkbox" id="import-redo">重新识别已完成的候选（人工修改仍受保护）</label>
<div class="import-drop" id="import-drop"><button class="button" id="import-choose">＋ 选择文件，可多选</button><span>也可拖入文件 · 每份 ≤ 32 MB · 每批 ≤ 20 份 / 128 MB</span></div>
<div id="import-queue" class="import-queue" aria-live="polite"></div>
<div class="import-actions"><button class="button primary" id="import-prepare">拆分并预览</button><button class="button" id="import-recognize-all">识别全部已拆分资料</button></div></div>
<section id="import-review" hidden><div class="import-review-heading"><div><h3 id="import-review-title"></h3><p id="import-review-hint" class="hint"></p></div><div><button class="button" id="import-discard-regions" hidden>放弃改动</button><button class="button" id="import-save-regions">保存拆分</button><button class="button primary" id="import-recognize-one">识别本份</button></div></div>
<details id="import-warnings" class="import-warning"><summary>分区提示</summary><ul></ul></details><div id="import-page-tabs" class="import-page-tabs"></div>
<div class="import-workbench"><div><p id="import-draw-hint" class="hint">在原页上拖动框出一道题，图形和选项也应包含在框内。每道题有独立颜色与编号；点击编号选中，拖动空白处新增。可在右侧修改坐标或删除重画。</p><div class="import-page" id="import-page"><img id="import-page-img" alt="原始页面预览"><svg id="import-overlay" role="img" aria-label="题目区域；可用鼠标拖动添加，下方坐标表也可编辑"></svg></div></div><div class="import-region-list" id="import-region-list"></div></div>
<div class="import-actions"><button class="button" id="import-open-library">到题库逐题核验 →</button></div></section>`;
document.body.append(importDialog);
const importTaskButton=document.createElement('button');importTaskButton.id='import-background-task';importTaskButton.className='import-background-task';importTaskButton.hidden=true;document.body.append(importTaskButton);
importTaskButton.addEventListener('click',()=>{importTaskButton.hidden=true;if(!importDialog.open)importDialog.showModal();});
function importOptions(){return {engine:appSettings.engine,layout:appSettings.layout,agent_assist:appSettings.agent_assist,allow_remote:$('#import-remote').checked,prepare_only:true,formula_enhanced:appSettings.formula_enhanced};}
function syncImportSettings(){const e=ingestion.engines.find(e=>e.id===appSettings.engine);$('#import-defaults').textContent=`${e?.name||appSettings.engine} · ${{auto:'自动版面',single:'单栏',columns:'双栏',manual:'手动框选'}[appSettings.layout]}${appSettings.engine==='paddle'?(appSettings.formula_enhanced?' · 公式精细':' · 快速文字'):''}${appSettings.agent_assist?' · Agent 校对':''}`;$('#import-remote').checked=false;$('#import-remote-line').hidden=!e?.remote;}
async function openImportDialog(){
  if(ingestion.busy){if(!importDialog.open)importDialog.showModal();importTaskButton.hidden=true;return;}
  const info=await api('/engines');ingestion.engines=info.imports.engines;
  await loadSettings();syncImportSettings();
  renderImportQueue();if(!importDialog.open)importDialog.showModal();
}
function renderImportQueue(){
  $('#import-queue').innerHTML=ingestion.items.length?ingestion.items.map((item,i)=>`<div class="import-file"><button data-import-item="${i}" ${item.id?'':'disabled'}><strong>${esc(item.name)}</strong><span>${esc(item.status||'待拆分')}</span></button><span class="hint">${item.file?(item.file.size/1024/1024).toFixed(1)+' MB':''}</span><button data-import-remove="${i}" aria-label="从队列移除 ${esc(item.name)}">×</button></div>`).join(''):'<p class="hint">每个文件分别处理；某一份失败时，其余文件可继续导入。</p>';
  $$('.import-controls input,.import-controls select,.import-controls button',importDialog).forEach(e=>e.disabled=ingestion.busy || (e.id==='import-recognize-all'&&!ingestion.items.some(x=>x.id)));
  $$('#import-review input,#import-review button,#import-queue button').forEach(e=>e.disabled=ingestion.busy);syncImportActions();
}
function addImportFiles(files){
  const list=Array.from(files);if(ingestion.items.length+list.length>20)throw new Error('每批最多 20 份文件，请处理完成后移除队列记录。');
  if(list.some(f=>f.size>32*1024*1024))throw new Error('每份文件不能超过 32 MB。');
  if([...ingestion.items.map(x=>x.file).filter(Boolean),...list].reduce((n,f)=>n+f.size,0)>128*1024*1024)throw new Error('本批文件总量不能超过 128 MB。');
  for(const file of list){if(!/\.(pdf|txt|md|markdown|png|jpe?g|webp|bmp)$/i.test(file.name))throw new Error('不支持的文件格式：'+file.name);}
  for(const file of list){if(!ingestion.items.some(x=>x.file&&x.file.name===file.name&&x.file.size===file.size&&x.file.lastModified===file.lastModified))ingestion.items.push({file,name:file.name,status:'待拆分'});}
  renderImportQueue();
}
function importErrorMessage(error){
 const text=String(error?.message||error||'处理失败');
 if(/超时|timed? out|deadline|504/i.test(text))return '识别超时，原图和已有结果已保留。可重试失败题目，或切换快速文字模式。';
 if(/429|503|busy|loading|not ready|繁忙|服务忙碌/i.test(text))return '识别服务忙碌或正在恢复，请稍后重试。';
 if(/fetch|network|连接|Failed to fetch/i.test(text))return '无法连接识别服务，请检查服务后重试。';
 if(/区域|拆分|同意|保护/.test(text)&&text.length<100)return text;
 return '处理失败，已有结果已保留。请重试或检查识别设置。';
}
function importStatus(message,error=false,detail=''){
 const el=$('#import-operation-status');el.hidden=!message;el.textContent=message;el.classList.toggle('error',error);
 const details=$('#import-error-details');details.hidden=!detail;details.open=false;$('pre',details).textContent=detail;
}
function syncImportActions(){
 const busy=ingestion.busy,active=ingestion.active,prepared=ingestion.items.filter(x=>x.id);
 $('#import-prepare').hidden=!ingestion.items.some(x=>!x.id);
 $('#import-recognize-all').hidden=prepared.length<2;
 $('#import-discard-regions').hidden=!ingestion.draft;
 $('#import-save-regions').disabled=busy||(!ingestion.draft&&ingestion.plan?.regions_confirmed);
 $('#import-recognize-one').disabled=busy||!active;
 $('#import-recognize-one').textContent=active?.failed?'重试失败题目':'开始识别';
}
function renderImportProgress(){
 const panel=$('#import-progress-panel');panel.hidden=!ingestion.started;if(!ingestion.started)return;
 const a=ingestion.aggregate,stage=ingestion.stage,seconds=Math.floor(((ingestion.busy?Date.now():ingestion.finishedAt||Date.now())-ingestion.started)/1000);
 const status=ingestion.busy?'running':ingestion.outcome||'succeeded';panel.dataset.status=status;panel.setAttribute('aria-busy',String(ingestion.busy));
 $('#import-progress-title').textContent=ingestion.busy?(a?.label||ingestion.progressLabel):({failed:'处理失败',partial:'部分完成',stopped:'已停止',succeeded:'处理完成'}[status]||'处理已结束');
 $('#import-progress-count').textContent=a?`${a.completed} / ${a.total} ${a.unit}已处理`:'';
 const total=$('#import-total-progress');if(a?.total>0){total.max=a.total;total.value=a.completed;}else if(!ingestion.busy){total.max=1;total.value=status==='succeeded'?1:0;}else total.removeAttribute('value');
 $('#import-progress-stage').textContent=!ingestion.busy?({failed:'本轮已结束，可重新尝试',partial:'失败项可重试，已完成项会跳过',stopped:'已保留当前结果'}[status]||'本轮处理完成'):stage?stage.phase+(stage.total?` · ${stage.completed} / ${stage.total} ${stage.unit}`:''):'等待处理';
 $('#import-progress-elapsed').textContent=`${ingestion.busy?'已用':'用时'} ${String(Math.floor(seconds/60)).padStart(2,'0')}:${String(seconds%60).padStart(2,'0')}`;
 const phase=$('#import-stage-progress');if(!ingestion.busy){phase.max=1;phase.value=status==='succeeded'?1:0;}else if(stage?.total>0){phase.max=stage.total;phase.value=stage.completed;}else phase.removeAttribute('value');
 $('.import-progress-actions',importDialog).hidden=!ingestion.busy;$('#import-background').hidden=!ingestion.busy;$('#import-stop').hidden=!ingestion.busy||!ingestion.canStop;$('#import-stop').disabled=ingestion.stopRequested;
 $('#import-stop').textContent=ingestion.stopRequested?'当前项结束后停止':'停止后续任务';
 if(!importDialog.open){importTaskButton.hidden=false;importTaskButton.textContent=(ingestion.busy?'◌ ':status==='succeeded'?'✓ ':'! ')+$('#import-progress-title').textContent+' · 查看';}
}
function importAggregate(label,completed,total,unit){ingestion.aggregate={label,completed,total,unit};renderImportProgress();}
async function importRequest(path,options={},timeoutMs=30000){
 const controller=new AbortController(),timer=setTimeout(()=>controller.abort(),timeoutMs);
 const tracked=options.method==='POST'&&(path==='/import'||path.endsWith('/regions')||path.endsWith('/recognize'));
 let pollTimer,pollController,finished=false,rejectProgress,misses=0;
 const failedProgress=new Promise((_,reject)=>rejectProgress=reject);
 try{
  if(tracked){
   const operation=await api('/operations',{method:'POST',signal:controller.signal});ingestion.stage={phase:'排队等待处理',completed:0,total:null};renderImportProgress();
   const poll=async()=>{if(finished)return;pollController=new AbortController();const deadline=setTimeout(()=>pollController.abort(),4000);
    try{const value=await api('/operations/'+operation.id,{signal:pollController.signal});misses=0;if(!finished){ingestion.stage=value;renderImportProgress();if(value.status==='failed'){rejectProgress(new Error(value.message||'后台处理失败'));controller.abort();}}}
    catch{if(!finished&&++misses>=3){rejectProgress(new Error('连接中断，无法确认后台处理结果。原件已保留；恢复连接后重新打开此来源确认结果。'));controller.abort();}}
    finally{clearTimeout(deadline);if(!finished)pollTimer=setTimeout(poll,700);}};
   pollTimer=setTimeout(poll,150);options={...options,headers:{...options.headers,'X-Paperdesk-Operation':operation.id}};
  }
  return await Promise.race([api(path,{...options,signal:controller.signal}),failedProgress]);
 }catch(error){if(error.name==='AbortError')throw new Error('请求超时，后台结果尚未确认。原件已保留；请重新打开此来源检查结果后重试。');throw error;}
 finally{finished=true;clearTimeout(timer);clearTimeout(pollTimer);pollController?.abort();}
}
async function importBusy(fn,label='处理中',canStop=true){
 if(ingestion.busy)return;ingestion.busy=true;ingestion.stopRequested=false;ingestion.canStop=canStop;ingestion.started=Date.now();ingestion.stage=null;ingestion.aggregate=null;ingestion.progressLabel=label;ingestion.outcome=null;
 importStatus('');renderImportProgress();const timer=setInterval(renderImportProgress,1000);renderImportQueue();
 try{const result=await fn();ingestion.outcome=result?.outcome||(result?.warning?'partial':ingestion.stopRequested?'stopped':'succeeded');importStatus(result?.warning||(ingestion.stopRequested?'已停止后续任务，结果已保留。':'处理完成。'),!!result?.warning,result?.detail||'');}
 catch(error){ingestion.outcome='failed';importStatus(importErrorMessage(error),true,error.message);}
 finally{clearInterval(timer);ingestion.finishedAt=Date.now();ingestion.busy=false;renderImportQueue();renderImportProgress();refreshMeta().catch(()=>{});}
}
async function prepareImportFiles(){await importBusy(async()=>{
  const pending=ingestion.items.filter(x=>!x.id);let handled=0;importAggregate('切分文件',0,pending.length,'份');
  let failures=0;const errors=[];
  for(const item of pending){
    if(ingestion.stopRequested)break;
    item.status='正在拆分与保存原图…';renderImportQueue();
    try{const form=new FormData();form.append('options',JSON.stringify(importOptions()));form.append('file',item.file);
      const result=await importRequest('/import',{method:'POST',body:form},240000);item.id=result.paper.id;item.status=result.count?`${result.count} 道候选 · 待确认范围`:'原件已保存 · 请查看提示';
      await selectImportItem(item);
    }catch(error){failures++;errors.push(item.name+': '+error.message);item.status='拆分失败 · 可重试';}
    handled++;importAggregate('切分文件',handled,pending.length,'份');renderImportQueue();
  }
  return failures?{outcome:failures===handled?'failed':'partial',warning:`${failures} 份拆分失败，可再次点击拆分。`,detail:errors.join('\n')}:undefined;
},'切分文件');}
async function selectImportItem(item){
  const data=await importRequest('/imports/'+encodeURIComponent(item.id));ingestion.active=item;ingestion.plan=data.plan;ingestion.questions=data.questions;ingestion.draft=false;
  ingestion.regions=data.questions.flatMap(q=>(q.origins||[]).filter(o=>o.paper_id===item.id&&o.rect).map(o=>({page:o.page,rect:{...o.rect}})));
  item.failed=data.questions.some(q=>q.recognition?.status==='error');
  ingestion.selected=-1;ingestion.page=data.plan.pages.some(p=>p.page===ingestion.page)?ingestion.page:(data.plan.pages[0]?.page||1);renderImportReview();
}
async function openImportPlan(id){
  if(!mayLeave())return;await openImportDialog();let item=ingestion.items.find(x=>x.id===id);
  if(!item){item={id,name:paperName(id),status:'已保存的导入记录'};ingestion.items.push(item);}
  try{await selectImportItem(item);renderImportQueue();}catch(error){throw new Error('此来源没有新版分区记录，请重新导入原件。'+error.message);}
}
function renderImportReview(){
  const item=ingestion.active;if(!item)return;$('#import-review').hidden=false;
  $('#import-review-title').textContent=item.name;$('#import-review-hint').textContent=ingestion.draft?'区域尚未保存；保存后再识别。':!ingestion.plan.regions_confirmed&&ingestion.plan.pages.length?'自动分区待确认：请核对题干、选项和配图，点击保存拆分后再识别。':`${ingestion.questions.length} 道候选 · 原件、页码和题目区域均保留`;
  const warnings=[...new Set(ingestion.plan.warnings||[])];$('#import-warnings').hidden=!warnings.length;$('#import-warnings summary').textContent=`分区提示 · ${warnings.length} 项`;$('#import-warnings ul').innerHTML=warnings.map(w=>`<li>${esc(w)}</li>`).join('');
  $('#import-page-tabs').innerHTML=ingestion.plan.pages.map(p=>`<button class="button ${p.page===ingestion.page?'active':''}" data-import-page="${p.page}">第 ${p.page} 页</button>`).join('');
  $$('#import-review input,#import-review button').forEach(e=>e.disabled=ingestion.busy);
  const page=ingestion.plan.pages.find(p=>p.page===ingestion.page);$('#import-page').hidden=!page;$('#import-draw-hint').hidden=!page;$('#import-save-regions').hidden=!page;
  if(page){const img=$('#import-page-img');const url=`/api/imports/${encodeURIComponent(item.id)}/pages/${page.page}`;if(img.getAttribute('src')!==url)img.src=url;
    const svg=$('#import-overlay');svg.setAttribute('viewBox',`0 0 ${page.width} ${page.height}`);
    drawImportOverlay(page);
  }
  syncImportActions();
  $('#import-region-list').innerHTML=page?ingestion.regions.map((r,i)=>r.page===ingestion.page?`<div class="import-region ${ingestion.selected===i?'selected':''}" style="--region-color:${regionColors[i%regionColors.length]}"><div><button class="region-select" data-region-select="${i}" aria-pressed="${ingestion.selected===i}"><span class="region-dot">${i+1}</span>题目区域 ${i+1}</button><button data-region-delete="${i}" aria-label="删除题目区域 ${i+1}">删除</button></div><div class="region-coordinates">${['x','y','width','height'].map(k=>`<label>${({x:'左',y:'上',width:'宽',height:'高'})[k]}<input type="number" min="${k==='width'||k==='height'?8:0}" step="1" value="${r.rect[k]}" data-region-index="${i}" data-region-key="${k}"></label>`).join('')}</div></div>`:'').join(''):
    ingestion.questions.map(q=>`<article class="import-text-question"><strong>题 ${esc(q.number||'未编号')}</strong><p>${esc(transcription(q).slice(0,200))}</p><span class="hint">${(q.origins||[]).map(o=>`p.${o.page} · 行 ${o.line_start}–${o.line_end}`).join(' / ')}</span></article>`).join('');
}
async function saveImportRegions(){
  if(!ingestion.draft&&(ingestion.plan.regions_confirmed||!ingestion.plan.pages.length))return;
  const data=await importRequest('/imports/'+ingestion.active.id+'/regions',{method:'POST',body:{revision:ingestion.plan.revision,regions:ingestion.regions}},90000);
  ingestion.plan=data.plan;ingestion.questions=data.questions;ingestion.active.status=`${data.questions.length} 道候选 · 已保存拆分`;ingestion.draft=false;renderImportReview();toast('拆分已保存，原图区域已逐题保留');
}
async function recognizeImportItem(item){
  item.lastError='';const retryOnly=!!item.failed;
  if(ingestion.draft)throw new Error('请先保存或重新载入当前拆分区域。');
  await selectImportItem(item);if(ingestion.plan.pages.length&&!ingestion.plan.regions_confirmed)throw new Error('请先核对自动分区并点击保存拆分，再识别本份。');let done=0,failed=0,protectedCount=0,handled=0;const total=ingestion.questions.length;importAggregate('识别题目',0,total,'题');
  for(const q of ingestion.questions){
    if(ingestion.stopRequested)break;
    if(retryOnly&&q.recognition&&!['pending','error'].includes(q.recognition.status)){done++;handled++;importAggregate('识别题目',handled,total,'题');continue;}
    if(q.revision>0||q.verified||q.status!=='pending'){protectedCount++;handled++;importAggregate('识别题目',handled,total,'题');continue;}
    const completed=['done','agent_reviewed'].includes(q.recognition?.status)||!q.recognition;
    if(completed&&!$('#import-redo').checked&&!appSettings.agent_assist){done++;handled++;importAggregate('识别题目',handled,total,'题');continue;}
    item.status=`正在逐题识别 ${done+failed+1} / ${ingestion.questions.length}`;renderImportQueue();
    try{const result=await importRequest('/questions/'+q.id+'/recognize',{method:'POST',body:{...importOptionsForProcessing(),only_agent:completed&&!$('#import-redo').checked&&appSettings.agent_assist}},330000);if(result.recognition?.status==='error'){failed++;item.lastError=result.recognition.error||'识别失败';}else done++;}
    catch(error){failed++;item.lastError=error.message;}
    handled++;importAggregate('识别题目',handled,total,'题');
    if(/HTTP (429|503|504)|服务连接失败或超时/.test(item.lastError)){ingestion.stopRequested=true;item.lastError+='；服务忙碌或正在恢复，剩余题目尚未识别，请待服务恢复后重试。';break;}
  }
  item.status=`${done} 题完成${failed?' · '+failed+' 题失败，可重试':''}${protectedCount?' · '+protectedCount+' 题受人工编辑保护':''}${appSettings.agent_assist?' · 已创建 Agent 校对任务':''}`;
  item.failed=failed>0;await selectImportItem(item);return failed?{outcome:done===0?'failed':'partial',warning:`${failed} 题识别失败，已完成的结果保留。可点击重试失败题目。`,detail:item.lastError}:undefined;
}
function importOptionsForProcessing(){const o=importOptions();return {engine:o.engine,allow_remote:o.allow_remote,agent_assist:o.agent_assist,formula_enhanced:o.formula_enhanced};}
function questionEvidence(q){
  const origins=q.origins||[];
  const details=origins.map((o,i)=>`<section class="question-evidence"><p class="hint">${esc(paperName(o.paper_id))} · p.${o.page}${o.line_start?' · 提取文字行 '+o.line_start+'–'+o.line_end:''}${o.rect?' · 区域 '+o.rect.x+','+o.rect.y+' / '+o.rect.width+'×'+o.rect.height:''}</p>${o.crop_id?`<a href="/api/questions/${encodeURIComponent(q.id)}/source/${i}" target="_blank" rel="noopener"><img alt="第 ${esc(q.number)} 题原始裁切区域，第 ${o.page} 页" src="/api/questions/${encodeURIComponent(q.id)}/source/${i}"></a>`:''}<a class="hint" href="/api/papers/${encodeURIComponent(o.paper_id)}/source#page=${o.page}" target="_blank" rel="noopener">打开原始文件 ↗</a></section>`).join('');
  const rec=q.recognition;const status=rec?`<p class="import-warning">${rec.status==='error'?esc(rec.error):rec.status==='pending'?'尚未识别；可先人工核对范围。':'识别引擎：'+esc(rec.engine)+' · 文字仍需人工核验'}</p>${rec.raw_text?`<details><summary>查看未编辑的识别原文</summary><pre>${esc(rec.raw_text)}</pre></details>`:''}`:'';
  return `<aside class="source-pane" hidden><div><span>原始证据 · 页码 / 区域</span><a href="${sourceLink(q)}" target="_blank" rel="noopener">打开原件 ↗</a></div>${status}${details||`<iframe title="原始试卷预览" src="${sourceLink(q)}"></iframe>`}</aside>`;
}
const regionColors=['#006bb3','#aa3e94','#007e68','#bc5700','#6151ba','#b82e48'];
let importDrag=null;
function drawImportOverlay(page){
  const scale=page.width/Math.max(1,$('#import-page').clientWidth), label=26*scale;
  $('#import-overlay').innerHTML=ingestion.regions.map((r,i)=>{
    if(r.page!==page.page)return '';const c=regionColors[i%regionColors.length],selected=i===ingestion.selected;
    return `<g class="region-shape ${selected?'selected':''}" style="color:${c}" data-region-select="${i}"><rect class="region-box" x="${r.rect.x}" y="${r.rect.y}" width="${r.rect.width}" height="${r.rect.height}"/><rect class="region-label" x="${r.rect.x}" y="${r.rect.y}" width="${label*1.4}" height="${label}" rx="${3*scale}"/><text x="${r.rect.x+label*.7}" y="${r.rect.y+label*.7}" style="font-size:${15*scale}px">${i+1}</text></g>`;
  }).join('');
}
function importPoint(e,page){const b=$('#import-overlay').getBoundingClientRect();return {x:Math.max(0,Math.min(page.width,Math.round((e.clientX-b.left)/b.width*page.width))),y:Math.max(0,Math.min(page.height,Math.round((e.clientY-b.top)/b.height*page.height)))};}
$('#import-overlay').addEventListener('pointerdown',e=>{
  if(ingestion.busy||e.button!==0)return;
  const selected=e.target.closest('[data-region-select]');
  if(selected){ingestion.selected=Number(selected.dataset.regionSelect);renderImportReview();return;}
  const p=ingestion.plan.pages.find(p=>p.page===ingestion.page);if(!p)return;
  importDrag={...importPoint(e,p),page:p};e.currentTarget.setPointerCapture(e.pointerId);
});
$('#import-overlay').addEventListener('pointermove',e=>{
  if(!importDrag)return;const d=importDrag,p=importPoint(e,d.page);let draft=$('#import-draft-box');
  if(!draft){draft=document.createElementNS('http://www.w3.org/2000/svg','rect');draft.id='import-draft-box';draft.classList.add('draft-box');e.currentTarget.append(draft);}
  for(const [k,v]of Object.entries({x:Math.min(p.x,d.x),y:Math.min(p.y,d.y),width:Math.abs(p.x-d.x),height:Math.abs(p.y-d.y)}))draft.setAttribute(k,v);
});
$('#import-overlay').addEventListener('pointerup',e=>{
  if(!importDrag)return;const d=importDrag;importDrag=null;$('#import-draft-box')?.remove();const p=importPoint(e,d.page);
  if(Math.abs(p.x-d.x)<8||Math.abs(p.y-d.y)<8)return;
  ingestion.regions.push({page:d.page.page,rect:{x:Math.min(p.x,d.x),y:Math.min(p.y,d.y),width:Math.abs(p.x-d.x),height:Math.abs(p.y-d.y)}});
  ingestion.selected=ingestion.regions.length-1;ingestion.draft=true;renderImportReview();
});
$('#import-overlay').addEventListener('pointercancel',()=>{importDrag=null;$('#import-draft-box')?.remove();});
new ResizeObserver(()=>{if(ingestion.plan){const page=ingestion.plan.pages.find(p=>p.page===ingestion.page);if(page)drawImportOverlay(page);}}).observe($('#import-page'));
$('#file').addEventListener('change',run(async e=>{try{addImportFiles(e.target.files);}finally{e.target.value='';}}));
$('#import-drop').addEventListener('dragover',e=>e.preventDefault());
$('#import-drop').addEventListener('drop',run(async e=>{e.preventDefault();if(!ingestion.busy)addImportFiles(e.dataTransfer.files);}));
importDialog.addEventListener('cancel',e=>{if(ingestion.busy||ingestion.draft){e.preventDefault();toast('请先完成当前处理或保存拆分范围',true);}});
importDialog.addEventListener('click',run(async e=>{const b=e.target.closest('button');if(!b)return;if(b.id==='import-background'){importDialog.close();renderImportProgress();return;}if(b.id==='import-stop'){ingestion.stopRequested=true;renderImportProgress();return;}if(ingestion.busy&&!b.dataset.close)return;
  if(b.dataset.close&&(ingestion.busy||ingestion.draft)){e.stopPropagation();throw new Error('请先完成当前处理或保存拆分范围');}
  if(b.dataset.importItem!==undefined){if(ingestion.draft)throw new Error('请先保存当前拆分范围');await selectImportItem(ingestion.items[Number(b.dataset.importItem)]);return;}
  if(b.dataset.importRemove!==undefined){if(ingestion.draft)throw new Error('请先保存当前拆分范围');const item=ingestion.items.splice(Number(b.dataset.importRemove),1)[0];if(item===ingestion.active){ingestion.active=null;$('#import-review').hidden=true;}renderImportQueue();return;}
  if(b.dataset.importPage){ingestion.page=Number(b.dataset.importPage);renderImportReview();return;}
  if(b.dataset.regionSelect!==undefined){ingestion.selected=Number(b.dataset.regionSelect);renderImportReview();return;}
  if(b.dataset.regionDelete!==undefined){ingestion.selected=-1;ingestion.regions.splice(Number(b.dataset.regionDelete),1);ingestion.draft=true;renderImportReview();return;}
  if(b.id==='import-choose')$('#file').click();
  if(b.id==='import-prepare')await prepareImportFiles();
  if(b.id==='import-discard-regions'){if(confirm('放弃尚未保存的范围改动？'))await selectImportItem(ingestion.active);}
  if(b.id==='import-save-regions')await importBusy(saveImportRegions,'保存拆分',false);
  if(b.id==='import-recognize-one')await importBusy(async()=>{const item=ingestion.active;try{return await recognizeImportItem(item);}catch(error){item.failed=true;item.status='识别未完成 · 可重试';throw error;}},'逐题识别');
  if(b.id==='import-recognize-all')await importBusy(async()=>{const warnings=[];for(const item of ingestion.items.filter(x=>x.id)){if(ingestion.stopRequested)break;try{const result=await recognizeImportItem(item);if(result?.warning)warnings.push(item.name+'：'+result.warning);}catch(error){item.failed=true;item.status='识别失败 · 可重试';warnings.push(item.name+'：'+importErrorMessage(error));}}return warnings.length?{warning:'部分资料未完成，请检查后重试。',detail:warnings.join('\n')}:undefined;});
  if(b.id==='import-open-library'){if(ingestion.draft)throw new Error('请先保存拆分范围');state.paper=ingestion.active.id;state.dirty=false;importDialog.close();await library();}
}));
importDialog.addEventListener('error',e=>importStatus(e.message,true));
importDialog.addEventListener('change',run(async e=>{if(e.target.dataset.regionIndex===undefined||ingestion.busy)return;const i=Number(e.target.dataset.regionIndex),key=e.target.dataset.regionKey,value=Number(e.target.value);if(!Number.isSafeInteger(value)||value<0)throw new Error('区域坐标需要非负整数');ingestion.regions[i].rect[key]=value;ingestion.draft=true;renderImportReview();}));
document.addEventListener('click',run(async e=>{const b=e.target.closest('[data-import-plan]');if(b)await openImportPlan(b.dataset.importPlan);}));

importDialog.addEventListener('close',run(async()=>{if(state.view==='library')await loadLibrary();}));

window.addEventListener('beforeunload',e=>{if(ingestion.busy||ingestion.draft){e.preventDefault();e.returnValue='';}});
