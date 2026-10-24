/** 会话内稳定身份历史；导航不重放业务命令。 */
export class NavigationHistory<T> {
 private items:T[]=[];
 private cursor=-1;
 private revision=0;
 private listeners=new Set<()=>void>();
 constructor(private key:(item:T)=>string,private limit=100){}
 subscribe=(listener:()=>void)=>{this.listeners.add(listener);return()=>{this.listeners.delete(listener);};};
 snapshot=()=>this.revision;
 private changed(){this.revision++;for(const listener of this.listeners)listener();}
 visit(item:T){if(this.cursor>=0&&this.key(this.items[this.cursor]!)===this.key(item))return;this.items=this.items.slice(0,this.cursor+1);this.items.push(item);if(this.items.length>this.limit)this.items.shift();this.cursor=this.items.length-1;this.changed();}
 private find(direction:-1|1,valid:(item:T)=>boolean){for(let i=this.cursor+direction;i>=0&&i<this.items.length;i+=direction)if(valid(this.items[i]!))return i;return -1;}
 canMove(direction:-1|1,valid:(item:T)=>boolean=()=>true){return this.find(direction,valid)>=0;}
 move(direction:-1|1,valid:(item:T)=>boolean=()=>true):T|null{const index=this.find(direction,valid);if(index<0)return null;this.cursor=index;this.changed();return this.items[index]!;}
}
