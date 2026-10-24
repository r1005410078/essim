/** 本项目绘制的24单位简化工程图标；主题是分类标记，不代表协议实现。 */
const folder='<path d="M3 6V4h6l3 3h9v13H3Z"/>';
const rect=(x:number,y:number,w:number,h:number)=>`<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="1"/>`;
const battery=rect(6,4,12,17)+'<path d="M10 4V2h4v2M9 9h6M12 6v6M9 16h6"/>';
const cabinet=rect(5,2,14,20)+'<path d="M7 8h10M7 14h10M16 16v3"/>';
const nodes='<path d="M12 7v5M5 17v-5h14v5"/>'+rect(9,2,6,5)+rect(2,17,6,5)+rect(16,17,6,5);
const wave='<path d="M2 12h4l3-7 6 14 3-7h4"/>';
const bolt='<path d="m13 3-7 10h6l-1 8 7-11h-6Z"/>';
const folderBadge=(body:string)=>folder+`<g transform="translate(9 9) scale(.48)">${body}</g>`;
const labelBadge=(text:string)=>folder+`<text x="12" y="16.5" text-anchor="middle" fill="currentColor" stroke="none" font-family="system-ui,sans-serif" font-size="${text.length>3?5:7}" font-weight="600">${text}</text>`;
const entries:[string,string,string][]=[
 ['folder','普通文件夹',folder],['project','项目文件夹',folderBadge(rect(4,2,15,20)+'<path d="M8 8h7M8 12h7M8 16h4"/>')],
 ['modbus-tcp','Modbus TCP',folderBadge(nodes)],['modbus-rtu','Modbus RTU',folderBadge('<path d="m2 7 3 12h14l3-12Z"/><path d="M7 10h.01M12 10h.01M17 10h.01M9 15h.01M15 15h.01"/>')],
 ['iec104','IEC 104',labelBadge('104')],['iec61850','IEC 61850',labelBadge('61850')],['mms','MMS',folderBadge(rect(2,4,20,16)+'<path d="m2 5 10 8L22 5"/>')],['goose','GOOSE',folderBadge(bolt)],
 ['sv','SV',folderBadge(wave)],['opcua','OPC UA',folderBadge(nodes)],['mqtt','MQTT',folderBadge('<circle cx="12" cy="12" r="2"/><path d="M7 7a7 7 0 0 0 0 10M17 7a7 7 0 0 1 0 10M3 3a13 13 0 0 0 0 18M21 3a13 13 0 0 1 0 18"/>')],['dnp3','DNP3',labelBadge('DNP3')],
 ['can','CAN',folderBadge('<path d="m12 2 9 5v10l-9 5-9-5V7Z"/>')],['bacnet','BACnet',folderBadge(nodes)],['dlt645','DL/T 645',labelBadge('645')],['custom','自定义协议',folderBadge('<circle cx="12" cy="12" r="7"/><path d="M12 2v4M12 18v4M2 12h4M18 12h4M9 12h6"/>')],
 ['station','储能电站','<path d="M2 21V9h7V3h6v18M2 21h20V7h-7M5 13h1M5 17h1"/>'+rect(17,11,3,6)],
 ['container','储能集装箱',rect(2,5,20,15)+'<path d="M6 8v9M10 8v9M14 8v9M18 8v9"/>'],['battery-container','电池舱',rect(2,5,20,15)+'<g transform="translate(8 7) scale(.5)">'+battery+'</g><path d="M5 8v9"/>'],
 ['cabinet','电池柜',cabinet],['rack','电池簇',rect(4,2,16,6)+rect(4,9,16,6)+rect(4,16,16,6)+'<path d="M7 5h2M7 12h2M7 19h2"/>'],['module','电池模组',rect(3,7,18,10)+'<path d="M1 10v4M23 10v4M7 10h10"/>'],['cell','电芯',battery],['bms','BMS',rect(3,4,10,16)+rect(15,8,7,9)+'<path d="M6 8h4M7 6v4M17 5v3M20 5v3M17 17v3M20 17v3"/>'],
 ['pcs','PCS',rect(3,3,18,18)+'<path d="m3 21 18-18M6 7h5M6 10h3M13 16q2-4 4 0t4 0"/>'],['ems','EMS',rect(2,3,20,14)+'<path d="M8 21h8M12 17v4M6 12V7h12v5M12 7V5"/>'],['meter','电表',rect(5,2,14,20)+rect(8,5,8,6)+'<path d="M9 16h.01M15 16h.01M12 19h.01"/>'],
 ['transformer','变压器',rect(3,6,7,14)+rect(14,6,7,14)+'<path d="M6 2v4M17 2v4M5 9v8M8 9v8M16 9v8M19 9v8"/>'],['switchgear','开关柜',cabinet+'<path d="m8 12 6-5M9 12h.01"/>'],['combiner','直流汇流箱',rect(3,5,18,15)+'<path d="M7 10h4M9 8v4M15 10h4M1 12h2M21 12h2"/>'],['breaker','断路器',rect(4,3,16,18)+'<path d="M9 3v18M15 3v18M6 8h1M11 15h2M17 8h1"/>'],['busbar','母线',rect(2,4,20,4)+'<path d="M6 8v12M12 8v12M18 8v12M4 20h4M10 20h4M16 20h4"/>'],
 ['solar','光伏阵列','<path d="m6 4-4 13h19L17 4ZM5 9h14M9 4 7 17M14 4l2 13M12 17v4M8 21h8"/>'],['wind','风机','<circle cx="12" cy="9" r="2"/><path d="M12 7V2c-4-1-5 4-2 6M14 10l6 3c3-3-1-6-5-5M10 10l-6 4c1 4 6 2 7-2M12 12v10"/>'],['grid','电网','<path d="m12 2-8 20h16ZM6 7h12M3 12h18M8 17h8M6 22l12-10M18 22 6 12"/>'],['load','交流负载',rect(2,4,20,16)+'<path d="M5 12q3-7 7 0t7 0"/>'],['charger','充电桩',rect(3,3,13,19)+bolt.replace('d="','transform="translate(3 5) scale(.45)" d="')+'<path d="M16 9h4v8h2V7M19 3v4M22 3v4"/>'],
 ['cooling','液冷系统',rect(4,4,16,16)+'<path d="M1 8h3M1 16h3M20 8h3M20 16h3M12 7c-2 3-4 5-4 7a4 4 0 0 0 8 0c0-2-2-4-4-7Z"/>'],['fire','消防系统',rect(7,8,10,14)+'<path d="M10 8V4h5V2h4M12 4V2M7 11H4V7h3M10 12h4"/>'],['temperature','温度传感器','<path d="M10 14V4a2 2 0 0 1 4 0v10a5 5 0 1 1-4 0ZM12 7v11"/>'],
 ['gateway','网关',rect(2,11,20,10)+'<path d="M12 11V3M9 7l3-4 3 4M6 16h.01M11 16h.01M16 16h.01"/>'],['switch','交换机',rect(2,7,20,12)+'<path d="M5 11h2v3H5ZM10 11h2v3h-2ZM15 11h2v3h-2M5 21v-2M19 21v-2"/>'],['serial-server','串口服务器',rect(2,6,20,14)+'<path d="m5 10 1 6h8l1-6ZM8 12h.01M11 12h.01M18 12h.01M18 16h.01"/>'],['table','采集点表',rect(3,2,18,20)+'<path d="M3 7h18M3 12h18M3 17h18M9 7v15M15 7v15"/>'],['measurement','遥测',wave],['indication','遥信','<path d="M2 16h5M7 16 17 6M17 16h5"/><circle cx="6" cy="16" r="2"/><circle cx="18" cy="16" r="2"/>'],['control','遥控','<rect x="2" y="6" width="20" height="12" rx="6"/><circle cx="15" cy="12" r="4"/>'],['adjustment','遥调','<path d="M2 5h20M2 12h20M2 19h20"/><circle cx="7" cy="5" r="2"/><circle cx="16" cy="12" r="2"/><circle cx="10" cy="19" r="2"/>'],
];
export const folderIcons=entries.map(([id,label,body],i)=>({id,label,body,group:i<16?'协议与项目':i<32?'储能设备':'系统与测试'}));
export const folderColors=['#477fbc','#399f91','#c58d36','#8c71bd','#7b8797'];
export const safeFolderColor=(color?:string)=>/^#[0-9a-f]{6}$/i.test(color??'')?color!:'#477fbc';
export const folderIcon=(id?:string)=>folderIcons.find(icon=>icon.id===id)??folderIcons[0]!;
export const folderSymbolId=(id:string)=>`folder-theme-${encodeURIComponent(id).replaceAll('%','_')}`;
export function FolderGlyph({icon,color,size=20}:{icon?:string;color?:string;size?:number}){return <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" style={{color:safeFolderColor(color),flexShrink:0}} aria-hidden="true" dangerouslySetInnerHTML={{__html:folderIcon(icon).body}}/>;}
export function folderSprite(folders:{id:string;icon?:string;color?:string}[]){return folders.map(f=>`<symbol id="${folderSymbolId(f.id)}" viewBox="0 0 24 24"><g fill="none" stroke="${safeFolderColor(f.color)}" color="${safeFolderColor(f.color)}" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">${folderIcon(f.icon).body}</g></symbol>`).join('');}
