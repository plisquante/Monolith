// node brand/src/render.js  (needs playwright + chromium): renders brand/*.svg to PNG
const {chromium}=require('playwright');const fs=require('fs'),path=require('path');
const B=path.resolve(__dirname,'..');
async function shot(b,svg,w,h,out,bg){const p=await b.newPage({viewport:{width:w,height:h}});
 const s=fs.readFileSync(path.join(B,svg),'utf8').replace(/<svg /,'<svg style="width:100%;height:100%;display:block" ');
 await p.setContent(`<body style="margin:0;background:${bg||'transparent'};width:${w}px;height:${h}px;image-rendering:pixelated">${s}</body>`);
 await p.screenshot({path:path.join(B,out),omitBackground:!bg});await p.close();}
(async()=>{const b=await chromium.launch();
 await shot(b,'monolith-icon-round.svg',1024,1024,'monolith-icon-1024.png');
 await shot(b,'monolith-title.svg',1024,256,'title.png');
 fs.mkdirSync(path.join(B,'android'),{recursive:true});
 for(const [d,px] of Object.entries({mdpi:48,hdpi:72,xhdpi:96,xxhdpi:144,xxxhdpi:192}))
   await shot(b,'monolith-icon-round.svg',px,px,`android/ic_launcher_${d}.png`);
 await shot(b,'monolith-logo.svg',1754,321,'../docs/logo.png','#1d1f27');
 await b.close()})();
