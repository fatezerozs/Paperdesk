/* Shared list selection and a separate transcription/review surface. */
'use strict';
const listSelections=Object.fromEntries(['library','quizzes','agents'].map(scope=>[scope,{mode:false,ids:new Set(),anchor:null}]));
let listBulkBusy=false;
const legacyOcrPlaceholder='【待识别】请先确认原图范围，再逐题识别或人工录入。';
function transcription(q){const text=String(q.content||'');return text.trim()===legacyOcrPlaceholder?'':text;}
function recognitionInfo(q){
  if(!transcription(q).trim())return {label:q.recognition?.status==='error'?'识别失败':hasOriginal(q)?'尚未识别':'尚未录入',detail:q.recognition?.status==='error'?(q.recognition.error||'请重试识别或人工录入。'):(hasOriginal(q)?'原件已保存，请先识别或人工录入，再核验题目。':'题干尚未录入，请先编辑题目。'),kind:'pending'};
  if(q.verified)return {label:'已人工核验',detail:'题干已人工核验。',kind:'verified'};
  return {label:q.recognition?'已识别 · 待核验':'待人工核验',detail:'请对照原件核对题干、公式、选项和图表；识别完成不代表审核通过。',kind:'pending'};
}
function listItems(scope){return {library:state.questions,quizzes:state.quizzes,agents:visibleAgentJobs()}[scope]||[];}
function selectionCheckbox(scope,id,label){return `<span class="select-slot"><input type="checkbox" data-list-check="${scope}" value="${esc(id)}" aria-label="选择 ${esc(label)}" tabindex="-1"></span>`;}
function selectionToolbar(scope){
  const operations=scope==='library'?'<button id="merge" disabled>合并所选</button><button data-bulk-action="quiz" disabled>用所选组卷</button>':scope==='agents'?'<button data-bulk-action="cancel" disabled>取消所选任务</button>':'';
  return `<div class="selection-toolbar" data-selection-scope="${scope}"><button class="button selection-toggle" data-select-mode="${scope}" aria-pressed="false">多选</button><span class="selection-hint">Ctrl / ⌘ 多选 · Shift 连选</span><div class="selection-controls" hidden><label class="select-all"><input type="checkbox" data-select-all="${scope}">全选当前列表</label><span class="selection-total" role="status" aria-live="polite"></span><button data-select-clear="${scope}">清空</button>${operations}<button data-bulk-action="export" disabled>导出所选</button>${scope!=='agents'?'<button class="danger" data-bulk-action="delete" disabled>删除所选</button>':''}</div></div>`;
}
function syncListSelection(scope){
  const selection=listSelections[scope],items=listItems(scope),valid=new Set(items.map(x=>x.id));
  selection.ids=new Set([...selection.ids].filter(id=>valid.has(id)));
  if(selection.anchor&&!valid.has(selection.anchor))selection.anchor=null;
  if(scope==='library')state.selected=[...selection.ids];
  const root=document.querySelector(`[data-selection-root="${scope}"]`),toolbar=document.querySelector(`[data-selection-scope="${scope}"]`);
  if(root){root.classList.toggle('selection-mode',selection.mode);root.querySelectorAll('[data-list-item]').forEach(row=>{
    const checked=selection.ids.has(row.dataset.listItem);row.classList.toggle('selected',checked);
    const checkbox=row.querySelector('[data-list-check]');checkbox.checked=checked;checkbox.tabIndex=selection.mode?0:-1;checkbox.disabled=listBulkBusy;
    row.querySelector('[data-item-open]')?.setAttribute('aria-pressed',String(checked));
  });}
  if(toolbar){
    const toggle=toolbar.querySelector('[data-select-mode]');toggle.textContent=selection.mode?'完成选择':'多选';toggle.setAttribute('aria-pressed',String(selection.mode));toggle.disabled=listBulkBusy;
    toolbar.querySelector('.selection-controls').hidden=!selection.mode;toolbar.querySelector('.selection-hint').hidden=selection.mode;
    toolbar.querySelector('.selection-total').textContent=`已选 ${selection.ids.size} / ${items.length}`;
    const all=toolbar.querySelector('[data-select-all]');all.checked=items.length>0&&selection.ids.size===items.length;all.indeterminate=selection.ids.size>0&&selection.ids.size<items.length;all.disabled=listBulkBusy||!items.length;
    toolbar.querySelectorAll('[data-bulk-action],#merge').forEach(button=>button.disabled=listBulkBusy||selection.ids.size<(button.id==='merge'?2:1));
    const clear=toolbar.querySelector('[data-select-clear]');clear.disabled=listBulkBusy||!selection.ids.size;
  }
}
function selectListItem(scope,id,event={}){
  const selection=listSelections[scope],ids=listItems(scope).map(x=>x.id);if(!ids.includes(id)||listBulkBusy)return;
  selection.mode=true;
  if(event.shiftKey&&selection.anchor&&ids.includes(selection.anchor)){
    const a=ids.indexOf(selection.anchor),b=ids.indexOf(id),range=ids.slice(Math.min(a,b),Math.max(a,b)+1);
    if(!event.ctrlKey&&!event.metaKey)selection.ids.clear();range.forEach(x=>selection.ids.add(x));
  }else{
    if(selection.ids.has(id))selection.ids.delete(id);else selection.ids.add(id);selection.anchor=id;
  }
  syncListSelection(scope);
}
function setAllSelected(scope,selected=true){const selection=listSelections[scope];selection.mode=true;selection.ids=new Set(selected?listItems(scope).map(x=>x.id):[]);if(!selection.anchor)selection.anchor=listItems(scope)[0]?.id||null;syncListSelection(scope);}
function renderLibraryRows(){
  const root=$('#questions');root.classList.add('selectable-list');root.dataset.selectionRoot='library';
  $('#library-selection').innerHTML=selectionToolbar('library');
  root.innerHTML=state.questions.length?state.questions.map(q=>{
    const info=recognitionInfo(q),text=transcription(q),origin=(q.origins||[]).findIndex(o=>o.crop_id);
    const status=q.status==='discarded'?'已舍弃':q.verified?'已核验':!text?'待识别':'待审核';
    return `<article class="question-row selectable-row compact-question" data-list="library" data-list-item="${esc(q.id)}">${selectionCheckbox('library',q.id,'第 '+(q.number||'未编号')+' 题')}<div class="question-main" role="button" tabindex="0" data-item-open data-preview="${esc(q.id)}"><div class="question-kicker"><span class="question-no">${esc(q.number||'—')}</span><span class="question-state ${q.verified?'verified':q.status==='discarded'?'discarded':'pending'}" title="${esc(info.detail)}">${status}</span><span class="question-subject">${q.subject==='未分类'?'':esc(q.subject)}</span><span class="difficulty-dots" title="难度 ${q.difficulty} / 5" aria-label="难度 ${q.difficulty} / 5">${[1,2,3,4,5].map(d=>`<i class="${d<=q.difficulty?'filled':''}"></i>`).join('')}</span></div>${text?`<div class="question-title">${markdown(text)}</div>`:origin>=0?`<div class="question-image-preview"><img loading="lazy" src="/api/questions/${encodeURIComponent(q.id)}/source/${origin}" alt="第 ${esc(q.number||'未编号')} 题原图"></div>`:'<p class="question-placeholder">点击录入题目</p>'}<div class="question-caption"><span class="question-origin" title="${esc(paperName(q.paper_id))}">${q.paper_id?esc(paperName(q.paper_id))+' · '+q.page_start+' 页':'独立题目'}</span>${q.tags.slice(0,2).map(t=>`<span class="mini-tag">${esc(t)}</span>`).join('')}${q.tags.length>2?`<span title="${esc(q.tags.join('、'))}">+${q.tags.length-2}</span>`:''}</div></div><div class="list-row-actions question-row-actions"><button class="row-preview" data-preview="${esc(q.id)}" title="预览与审核" aria-label="预览与审核">◉</button><button class="row-end" data-edit="${esc(q.id)}" title="编辑题目" aria-label="编辑题目">↗</button></div></article>`;
  }).join(''):empty('这里，留给下一道好题。',Object.keys(filter()).length?'没有符合筛选条件的题目，请调整筛选或导入试卷。':'导入图片、PDF 或文字资料，先拆题再逐一审阅，也可以手工建题。','<button class="button" data-action="import">＋ 导入第一份试卷</button>');
  syncListSelection('library');
}
async function openListItem(row){
  if(row.dataset.list==='library')await previewQuestion(row.dataset.listItem);
  else if(row.dataset.list==='quizzes')await openQuiz(row.dataset.listItem);
  else{const job=state.jobs.find(j=>j.id===row.dataset.listItem);if(job)await previewQuestion(job.question_id);}
}
async function runBulkAction(scope,action){
  if(listBulkBusy)return;const selection=listSelections[scope],ids=[...selection.ids],items=listItems(scope).filter(x=>selection.ids.has(x.id));if(!ids.length)return;
  if(action==='export'){download(`paperdesk-${scope}-selected.json`,JSON.stringify({kind:scope,items},null,2));toast(`已导出 ${items.length} 项`);return;}
  if(action==='quiz'){await openAdvancedQuiz();const form=$('#quiz-form');form.elements.selected_only.checked=true;form.elements.count.value=String(Math.min(5,ids.length));await previewQuiz();return;}
  const targets=action==='cancel'?items.filter(j=>['queued','running','review'].includes(j.status)):items;
  if(!targets.length){toast('所选任务均已结束，无需取消');return;}
  if(!confirm(action==='cancel'?`取消所选的 ${targets.length} 项未结束任务？`:`永久删除所选 ${targets.length} ${scope==='library'?'道题目？已保存的测验快照会保留。':'份测验？题库中的题目会保留。'}`))return;
  listBulkBusy=true;syncListSelection(scope);let success=0;const errors=[];
  try{
    for(const item of targets){
      try{await api(`/${{library:'questions',quizzes:'quizzes',agents:'jobs'}[scope]}/${encodeURIComponent(item.id)}${action==='cancel'?'/reject':''}`,{method:action==='cancel'?'POST':'DELETE'});selection.ids.delete(item.id);success++;}
      catch(error){errors.push(error.message);}
    }
    await refreshMeta();if(state.view===scope){if(scope==='library')await loadLibrary();else if(scope==='quizzes')await renderQuizzes();else await renderAgents();}
    toast(`${success} 项${action==='cancel'?'已取消':'已删除'}${errors.length?'；'+errors.length+' 项失败，已保留选择：'+errors[0]:''}`,errors.length>0);
  }finally{listBulkBusy=false;syncListSelection(scope);}
}
document.addEventListener('click',async e=>{
  try{
    const row=e.target.closest('[data-list-item]'),toolbar=e.target.closest('[data-selection-scope]');
    if(row){
      if(e.target.closest('a,textarea,select'))return;
      const checkbox=e.target.closest('[data-list-check]'),action=e.target.closest('.list-row-actions');
      if(checkbox||e.ctrlKey||e.metaKey||e.shiftKey||(listSelections[row.dataset.list].mode&&!action)){
        if(!checkbox)e.preventDefault();e.stopImmediatePropagation();selectListItem(row.dataset.list,row.dataset.listItem,e);queueMicrotask(()=>syncListSelection(row.dataset.list));return;
      }
      if(e.target.closest('[data-item-open]')){e.preventDefault();e.stopImmediatePropagation();await openListItem(row);return;}
    }
    if(toolbar){
      const target=e.target.closest('button,input');if(!target)return;if(target.tagName!=='INPUT')e.preventDefault();e.stopImmediatePropagation();if(listBulkBusy)return;
      const scope=toolbar.dataset.selectionScope,selection=listSelections[scope];
      if(target.dataset.selectMode){selection.mode=!selection.mode;if(!selection.mode){selection.ids.clear();selection.anchor=null;}syncListSelection(scope);}
      else if(target.dataset.selectAll){setAllSelected(scope,selection.ids.size!==listItems(scope).length);queueMicrotask(()=>syncListSelection(scope));}
      else if(target.dataset.selectClear){selection.ids.clear();syncListSelection(scope);}
      else if(target.dataset.bulkAction)await runBulkAction(scope,target.dataset.bulkAction);
      else if(target.id==='merge'){if(selection.ids.size<2||!confirm('按选择顺序合并题目？合并后需重新核验，原答案和解析会清空。'))return;await api('/questions/merge',{method:'POST',body:{ids:[...selection.ids]}});await library();toast('已合并，请重新核验');}
    }
  }catch(error){toast(error.message,true);}
},true);
document.addEventListener('keydown',async e=>{
  if(document.querySelector('dialog[open]')||e.target.closest('textarea,select,input:not([type=checkbox]):not([type=radio])')||e.target.isContentEditable)return;
  const scope=state.view,selection=listSelections[scope];if(!selection||!document.querySelector(`[data-selection-root="${scope}"]`))return;
  if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='a'){e.preventDefault();e.stopImmediatePropagation();if(!listBulkBusy)setAllSelected(scope);return;}
  if(e.key==='Escape'&&selection.mode){e.preventDefault();selection.mode=false;selection.ids.clear();selection.anchor=null;syncListSelection(scope);return;}
  const row=e.target.closest('[data-list-item]');if(!row||!e.target.closest('[data-item-open]'))return;
  if(e.key===' '||(e.key==='Enter'&&(e.ctrlKey||e.metaKey||e.shiftKey))){e.preventDefault();e.stopImmediatePropagation();if(!e.repeat)selectListItem(scope,row.dataset.listItem,e);}
  else if(e.key==='Enter'){e.preventDefault();try{if(selection.mode)selectListItem(scope,row.dataset.listItem,e);else await openListItem(row);}catch(error){toast(error.message,true);}}
},true);

const reviewDialog=document.createElement('dialog');reviewDialog.id='review-dialog';reviewDialog.className='review-dialog';reviewDialog.setAttribute('aria-labelledby','review-title');document.body.append(reviewDialog);
let reviewContext=null;
async function previewQuestion(id){
  const q=await api('/questions/'+encodeURIComponent(id));let ids=state.view==='library'?state.questions.map(x=>x.id):[id];if(!ids.includes(id))ids=[id];
  reviewContext={q,ids,index:ids.indexOf(id),busy:false};renderQuestionReview();if(!reviewDialog.open)reviewDialog.showModal();
}
function renderQuestionReview(){
  const {q,ids,index,busy}=reviewContext,info=recognitionInfo(q),text=transcription(q),approved=q.verified&&q.status==='valuable';
  const evidence=(q.origins||[]).map((o,i)=>`<section class="review-evidence"><div class="review-evidence-caption">${esc(paperName(o.paper_id))} · 第 ${o.page} 页</div>${o.crop_id?`<a href="/api/questions/${encodeURIComponent(q.id)}/source/${i}" target="_blank" rel="noopener"><img src="/api/questions/${encodeURIComponent(q.id)}/source/${i}" alt="第 ${esc(q.number||'未编号')} 题原始裁切区域"></a>`:''}</section>`).join('');
  reviewDialog.innerHTML=`<div class="dialog-heading"><div><p class="eyebrow">QUESTION REVIEW</p><h2 id="review-title">${esc(q.number?'第 '+q.number+' 题':'题目预览')}</h2><p class="hint">${esc(paperName(q.paper_id))} · 修订 ${q.revision}</p></div><button data-review-close aria-label="关闭预览" ${busy?'disabled':''}>×</button></div><div class="review-state ${approved?'approved':q.status==='discarded'?'discarded':''}" role="status"><strong>${approved?'已审核通过 · 已保留':q.status==='discarded'?'已舍弃':info.label}</strong><span>${esc(approved?'此题可参与已核验题目的随机组卷。':q.status==='discarded'?'此题已从可抽题范围中排除。':info.detail)}</span></div><div class="review-body ${hasOriginal(q)&&text?'with-original':''} ${hasOriginal(q)&&!text?'image-only':''}"><article class="review-transcription" ${hasOriginal(q)&&!text?'hidden':''}><h3>题干</h3>${text?`<div class="prose-preview">${markdown(text)}</div>`:'<div class="review-empty-text"><span>—</span><p>暂无题干文字</p></div>'}${q.answer||q.explanation?`<details><summary>答案与解析</summary><div class="prose-preview">${markdown(q.answer)}\n\n${markdown(q.explanation)}</div></details>`:''}</article>${hasOriginal(q)?`<aside class="review-original"><div class="review-original-heading"><h3>原题对照</h3><a href="${sourceLink(q)}" target="_blank" rel="noopener">打开原件 ↗</a></div>${evidence||`<iframe title="原始试卷对照" src="${sourceLink(q)}"></iframe>`}</aside>`:''}</div><footer class="review-footer"><label class="checkline"><input id="review-confirm" type="checkbox" ${!text||approved||busy?'disabled':''}>我已核对题干、公式、选项和图表</label><div class="review-actions"><div><button class="button" data-review-move="-1" ${index===0||busy?'disabled':''}>← 上一题</button><button class="button" data-review-move="1" ${index===ids.length-1||busy?'disabled':''}>下一题 →</button></div><div><button class="button" data-rereview="${esc(q.id)}" ${busy?'disabled':''}>Agent 校对</button><button class="button" data-review-edit ${busy?'disabled':''}>${q.status==='pending'?'编辑题目':'退回修改'}</button>${!text&&q.origins?.length?`<button class="button" data-review-import ${busy?'disabled':''}>拆分与识别</button>`:''}<button class="button danger" data-review-decision="discard" ${q.status==='discarded'||busy?'disabled':''}>舍弃此题</button><button class="button primary" data-review-decision="approve" disabled>${approved?'已通过':'通过并保留'}</button></div></div></footer>`;
  reviewDialog.querySelectorAll('.review-evidence img').forEach(img=>img.addEventListener('error',()=>{img.hidden=true;const notice=document.createElement('p');notice.className='hint';notice.textContent='原图加载失败，请打开原件核对。';img.parentElement.after(notice);},{once:true}));
}
reviewDialog.addEventListener('change',e=>{if(e.target.id==='review-confirm')reviewDialog.querySelector('[data-review-decision="approve"]').disabled=!e.target.checked||reviewContext.busy;});
reviewDialog.addEventListener('cancel',e=>{if(reviewContext?.busy)e.preventDefault();});
reviewDialog.addEventListener('click',async e=>{
  const button=e.target.closest('button');if(!button||reviewContext?.busy)return;
  try{
    if(button.hasAttribute('data-review-close')){reviewDialog.close();return;}
    if(button.dataset.reviewMove){const index=reviewContext.index+Number(button.dataset.reviewMove);if(index<0||index>=reviewContext.ids.length)return;const q=await api('/questions/'+reviewContext.ids[index]);reviewContext={...reviewContext,q,index};renderQuestionReview();reviewDialog.querySelector('.review-body').scrollTop=0;return;}
    if(button.hasAttribute('data-review-import')){const id=reviewContext.q.origins[0].paper_id;reviewDialog.close();await openImportPlan(id);return;}
    if(button.hasAttribute('data-review-edit')){
      if(reviewContext.q.status!=='pending'||reviewContext.q.verified){reviewContext.busy=true;reviewContext.q=await api('/questions/'+reviewContext.q.id+'/review',{method:'POST',body:{revision:reviewContext.q.revision,decision:'return'}});}
      const id=reviewContext.q.id;reviewDialog.close();await edit(id);return;
    }
    if(button.dataset.reviewDecision){
      const decision=button.dataset.reviewDecision;if(decision==='approve'&&!reviewDialog.querySelector('#review-confirm').checked)return;
      reviewContext.busy=true;reviewDialog.querySelectorAll('button,input').forEach(x=>x.disabled=true);
      reviewContext.q=await api('/questions/'+reviewContext.q.id+'/review',{method:'POST',body:{revision:reviewContext.q.revision,decision}});
      await refreshMeta();if(state.view==='library')await loadLibrary();toast(decision==='approve'?'审核通过，已保留此题':'此题已舍弃');
    }
  }catch(error){toast(error.message,true);}
  finally{if(reviewContext){reviewContext.busy=false;if(reviewDialog.open)renderQuestionReview();}}
});
