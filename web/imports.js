'use strict';
const ingestion = {items:[],active:null,page:1,regions:[],draft:false,busy:false,engines:[],selected:-1};
const importDialog = document.createElement('dialog');
importDialog.id='import-dialog';
importDialog.innerHTML=`<div class="dialog-heading"><div><p class="eyebrow">IMPORT / REVIEW / RECOGNIZE</p><h2>先分好题，再留下文字。</h2></div><button data-close="import-dialog" aria-label="关闭导入">×</button></div>
<p class="muted">多选 PDF、图片、Markdown 或 TXT。原件与每道题的原图区域会一起保留，方便日后对照。</p>
<div class="import-controls"><div class="import-options"><label>识别方式<select id="import-engine"><option value="local">本地 Tesseract · 离线</option></select></label><label>图片版面<select id="import-layout"><option value="auto">自动判断</option><option value="single">单栏</option><option value="columns">双栏</option><option value="manual">手动框选 · 不预识别</option></select></label></div>
<label class="checkline"><input type="checkbox" id="import-formula" checked>公式增强：PDF 按原图拆题，避免文字层丢失数学符号</label><p class="hint">数学试卷建议搭配“PaddleOCR · 正文与公式”或 Mathpix。普通本地 OCR 不具备专用公式识别能力。</p>
<label class="checkline"><input type="checkbox" id="import-agent">Agent 辅助识别与校对</label><p class="hint">开启后创建逐题识别任务，由接入的 agent 读取原题区域。没有接入 worker 时，任务等待领取；结果需人工采纳。</p>
<label class="checkline" id="import-remote-line" hidden><input type="checkbox" id="import-remote">同意将本次题目图片发送给所选外部 OCR 服务</label>
<label class="checkline"><input type="checkbox" id="import-redo">重新识别已完成的候选（人工修改仍受保护）</label>
<div class="import-drop" id="import-drop"><button class="button" id="import-choose">＋ 选择文件，可多选</button><span>也可拖入文件 · 每份 ≤ 32 MB · 每批 ≤ 20 份 / 128 MB</span></div>
<div id="import-queue" class="import-queue" aria-live="polite"></div>
<div class="import-actions"><button class="button primary" id="import-prepare">拆分并预览</button><button class="button" id="import-recognize-all">识别全部已拆分资料</button></div></div>
<section id="import-review" hidden><div class="import-review-heading"><div><h3 id="import-review-title"></h3><p id="import-review-hint" class="hint"></p></div><div><button class="button" id="import-save-regions">保存拆分</button><button class="button primary" id="import-recognize-one">识别本份</button></div></div>
<div id="import-warnings" class="import-warning"></div><div id="import-page-tabs" class="import-page-tabs"></div>
<div class="import-workbench"><div><p id="import-draw-hint" class="hint">在原页上拖动框出一道题，图形和选项也应包含在框内。每道题有独立颜色与编号；点击编号选中，拖动空白处新增。可在右侧修改坐标或删除重画。</p><div class="import-page" id="import-page"><img id="import-page-img" alt="原始页面预览"><svg id="import-overlay" role="img" aria-label="题目区域；可用鼠标拖动添加，下方坐标表也可编辑"></svg></div></div><div class="import-region-list" id="import-region-list"></div></div>
<div class="import-actions"><button class="button" id="import-open-library">到题库逐题核验 →</button></div></section>`;
document.body.append(importDialog);
function importOptions(){return {engine:$('#import-engine').value,layout:$('#import-layout').value,agent_assist:$('#import-agent').checked,allow_remote:$('#import-remote').checked,prepare_only:true,formula_enhanced:$('#import-formula').checked};}
async function openImportDialog(){
  const info=await api('/engines');ingestion.engines=info.imports.engines;
  $('#import-engine').innerHTML=ingestion.engines.map(e=>`<option value="${esc(e.id)}" ${e.configured?'':'disabled'}>${esc(e.name)}${e.configured?'':' · 尚未配置'}</option>`).join('');
  $('#import-remote').checked=false;const preferred=ingestion.engines.find(e=>e.id==='paddle'&&e.configured);if(preferred)$('#import-engine').value=preferred.id;$('#import-remote-line').hidden=!ingestion.engines.find(e=>e.id===$('#import-engine').value)?.remote;
  renderImportQueue();if(!importDialog.open)importDialog.showModal();
}
function renderImportQueue(){
  $('#import-queue').innerHTML=ingestion.items.length?ingestion.items.map((item,i)=>`<div class="import-file"><button data-import-item="${i}" ${item.id?'':'disabled'}><strong>${esc(item.name)}</strong><span>${esc(item.status||'待拆分')}</span></button><span class="hint">${item.file?(item.file.size/1024/1024).toFixed(1)+' MB':''}</span><button data-import-remove="${i}" aria-label="从队列移除 ${esc(item.name)}">×</button></div>`).join(''):'<p class="hint">每个文件分别处理；某一份失败时，其余文件可继续导入。</p>';
  $$('.import-controls input,.import-controls select,.import-controls button',importDialog).forEach(e=>e.disabled=ingestion.busy || (e.id==='import-recognize-all'&&!ingestion.items.some(x=>x.id)));
  $$('#import-review button').forEach(e=>e.disabled=ingestion.busy);
}
function addImportFiles(files){
  const list=Array.from(files);if(ingestion.items.length+list.length>20)throw new Error('每批最多 20 份文件，请处理完成后移除队列记录。');
  if(list.some(f=>f.size>32*1024*1024))throw new Error('每份文件不能超过 32 MB。');
  if([...ingestion.items.map(x=>x.file).filter(Boolean),...list].reduce((n,f)=>n+f.size,0)>128*1024*1024)throw new Error('本批文件总量不能超过 128 MB。');
  for(const file of list){if(!/\.(pdf|txt|md|markdown|png|jpe?g|webp|bmp)$/i.test(file.name))throw new Error('不支持的文件格式：'+file.name);}
  for(const file of list){if(!ingestion.items.some(x=>x.file&&x.file.name===file.name&&x.file.size===file.size&&x.file.lastModified===file.lastModified))ingestion.items.push({file,name:file.name,status:'待拆分'});}
  renderImportQueue();
}
async function importBusy(fn){
  if(ingestion.busy)return;ingestion.busy=true;renderImportQueue();
  try{await fn();}finally{ingestion.busy=false;renderImportQueue();await refreshMeta();}
}
async function prepareImportFiles(){await importBusy(async()=>{
  for(const item of ingestion.items.filter(x=>!x.id)){
    item.status='正在拆分与保存原图…';renderImportQueue();
    try{const form=new FormData();form.append('options',JSON.stringify(importOptions()));form.append('file',item.file);
      const result=await api('/import',{method:'POST',body:form});item.id=result.paper.id;item.status=result.count?`${result.count} 道候选 · 待确认范围`:'原件已保存 · 请查看提示';
      await selectImportItem(item);
    }catch(error){item.status='导入失败：'+error.message;}
    renderImportQueue();
  }
});}
async function selectImportItem(item){
  const data=await api('/imports/'+encodeURIComponent(item.id));ingestion.active=item;ingestion.plan=data.plan;ingestion.questions=data.questions;ingestion.draft=false;
  ingestion.regions=data.questions.flatMap(q=>(q.origins||[]).filter(o=>o.paper_id===item.id&&o.rect).map(o=>({page:o.page,rect:{...o.rect}})));
  ingestion.selected=-1;ingestion.page=data.plan.pages[0]?.page||1;renderImportReview();
}
async function openImportPlan(id){
  if(!mayLeave())return;await openImportDialog();let item=ingestion.items.find(x=>x.id===id);
  if(!item){item={id,name:paperName(id),status:'已保存的导入记录'};ingestion.items.push(item);}
  try{await selectImportItem(item);renderImportQueue();}catch(error){throw new Error('此来源没有新版分区记录，请重新导入原件。'+error.message);}
}
function renderImportReview(){
  const item=ingestion.active;if(!item)return;$('#import-review').hidden=false;
  $('#import-review-title').textContent=item.name;$('#import-review-hint').textContent=ingestion.draft?'区域尚未保存；保存后再识别。':`${ingestion.questions.length} 道候选 · 原件、页码和题目区域均保留`;
  $('#import-warnings').textContent=ingestion.plan.warnings.join(' ');
  $('#import-page-tabs').innerHTML=ingestion.plan.pages.map(p=>`<button class="button ${p.page===ingestion.page?'active':''}" data-import-page="${p.page}">第 ${p.page} 页</button>`).join('');
  $$('#import-review input,#import-review button').forEach(e=>e.disabled=ingestion.busy);
  const page=ingestion.plan.pages.find(p=>p.page===ingestion.page);$('#import-page').hidden=!page;$('#import-draw-hint').hidden=!page;$('#import-save-regions').hidden=!page;
  if(page){const img=$('#import-page-img');const url=`/api/imports/${encodeURIComponent(item.id)}/pages/${page.page}`;if(img.getAttribute('src')!==url)img.src=url;
    const svg=$('#import-overlay');svg.setAttribute('viewBox',`0 0 ${page.width} ${page.height}`);
    drawImportOverlay(page);
  }
  $('#import-region-list').innerHTML=page?ingestion.regions.map((r,i)=>r.page===ingestion.page?`<div class="import-region ${ingestion.selected===i?'selected':''}" style="--region-color:${regionColors[i%regionColors.length]}"><div><button class="region-select" data-region-select="${i}" aria-pressed="${ingestion.selected===i}"><span class="region-dot">${i+1}</span>题目区域 ${i+1}</button><button data-region-delete="${i}" aria-label="删除题目区域 ${i+1}">删除</button></div><div class="region-coordinates">${['x','y','width','height'].map(k=>`<label>${({x:'左',y:'上',width:'宽',height:'高'})[k]}<input type="number" min="${k==='width'||k==='height'?8:0}" step="1" value="${r.rect[k]}" data-region-index="${i}" data-region-key="${k}"></label>`).join('')}</div></div>`:'').join(''):
    ingestion.questions.map(q=>`<article class="import-text-question"><strong>题 ${esc(q.number||'未编号')}</strong><p>${esc(transcription(q).slice(0,200))}</p><span class="hint">${(q.origins||[]).map(o=>`p.${o.page} · 行 ${o.line_start}–${o.line_end}`).join(' / ')}</span></article>`).join('');
}
async function saveImportRegions(){
  if(!ingestion.draft)return;
  const data=await api('/imports/'+ingestion.active.id+'/regions',{method:'POST',body:{revision:ingestion.plan.revision,regions:ingestion.regions}});
  ingestion.plan=data.plan;ingestion.questions=data.questions;ingestion.active.status=`${data.questions.length} 道候选 · 已保存拆分`;ingestion.draft=false;renderImportReview();toast('拆分已保存，原图区域已逐题保留');
}
async function recognizeImportItem(item){
  item.lastError='';
  if(ingestion.draft)throw new Error('请先保存或重新载入当前拆分区域。');
  await selectImportItem(item);let done=0,failed=0,protectedCount=0;
  for(const q of ingestion.questions){
    if(q.revision>0||q.verified||q.status!=='pending'){protectedCount++;continue;}
    const completed=['done','agent_reviewed'].includes(q.recognition?.status)||!q.recognition;
    if(completed&&!$('#import-redo').checked&&!$('#import-agent').checked){done++;continue;}
    item.status=`正在逐题识别 ${done+failed+1} / ${ingestion.questions.length}`;renderImportQueue();
    try{const result=await api('/questions/'+q.id+'/recognize',{method:'POST',body:{...importOptionsForProcessing(),only_agent:completed&&!$('#import-redo').checked&&$('#import-agent').checked}});if(result.recognition?.status==='error')failed++;else done++;}
    catch(error){failed++;item.lastError=error.message;}
  }
  item.status=`${done} 题完成${failed?' · '+failed+' 题失败，可重试':''}${protectedCount?' · '+protectedCount+' 题受人工编辑保护':''}${$('#import-agent').checked?' · 已创建 Agent 校对任务':''}`;
  if(item.lastError)item.status+=' · '+item.lastError;await selectImportItem(item);
}
function importOptionsForProcessing(){const o=importOptions();return {engine:o.engine,allow_remote:o.allow_remote,agent_assist:o.agent_assist};}
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
$('#import-engine').addEventListener('change',()=>{$('#import-remote').checked=false;$('#import-remote-line').hidden=!ingestion.engines.find(e=>e.id===$('#import-engine').value)?.remote;});
importDialog.addEventListener('cancel',e=>{if(ingestion.busy||ingestion.draft){e.preventDefault();toast('请先完成当前处理或保存拆分范围',true);}});
importDialog.addEventListener('click',run(async e=>{const b=e.target.closest('button');if(!b)return;
  if(b.dataset.close&&(ingestion.busy||ingestion.draft)){e.stopPropagation();throw new Error('请先完成当前处理或保存拆分范围');}
  if(b.dataset.importItem!==undefined){if(ingestion.draft)throw new Error('请先保存当前拆分范围');await selectImportItem(ingestion.items[Number(b.dataset.importItem)]);return;}
  if(b.dataset.importRemove!==undefined){if(ingestion.draft)throw new Error('请先保存当前拆分范围');const item=ingestion.items.splice(Number(b.dataset.importRemove),1)[0];if(item===ingestion.active){ingestion.active=null;$('#import-review').hidden=true;}renderImportQueue();return;}
  if(b.dataset.importPage){ingestion.page=Number(b.dataset.importPage);renderImportReview();return;}
  if(b.dataset.regionSelect!==undefined){ingestion.selected=Number(b.dataset.regionSelect);renderImportReview();return;}
  if(b.dataset.regionDelete!==undefined){ingestion.selected=-1;ingestion.regions.splice(Number(b.dataset.regionDelete),1);ingestion.draft=true;renderImportReview();return;}
  if(b.id==='import-choose')$('#file').click();
  if(b.id==='import-prepare')await prepareImportFiles();
  if(b.id==='import-save-regions')await importBusy(saveImportRegions);
  if(b.id==='import-recognize-one')await importBusy(()=>recognizeImportItem(ingestion.active));
  if(b.id==='import-recognize-all')await importBusy(async()=>{for(const item of ingestion.items.filter(x=>x.id))await recognizeImportItem(item);});
  if(b.id==='import-open-library'){if(ingestion.draft)throw new Error('请先保存拆分范围');state.paper=ingestion.active.id;state.dirty=false;importDialog.close();await library();}
}));
importDialog.addEventListener('change',run(async e=>{if(e.target.dataset.regionIndex===undefined||ingestion.busy)return;const i=Number(e.target.dataset.regionIndex),key=e.target.dataset.regionKey,value=Number(e.target.value);if(!Number.isSafeInteger(value)||value<0)throw new Error('区域坐标需要非负整数');ingestion.regions[i].rect[key]=value;ingestion.draft=true;renderImportReview();}));
document.addEventListener('click',run(async e=>{const b=e.target.closest('[data-import-plan]');if(b)await openImportPlan(b.dataset.importPlan);}));

importDialog.addEventListener('close',run(async()=>{if(state.view==='library')await loadLibrary();}));
