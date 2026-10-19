"""分层软件规模探测；不将小点表探测等同于典型亿点容量。
依赖verify_tcp_slave_runtime.py及PyModbus。所有地址／值均为测试约定。
"""
import argparse,json,socket,subprocess,tempfile,time,platform,statistics,resource
from pathlib import Path
from verify_tcp_slave_runtime import snapshot,port
from pymodbus.client import ModbusTcpClient

def main():
 soft,hard=resource.getrlimit(resource.RLIMIT_NOFILE);resource.setrlimit(resource.RLIMIT_NOFILE,(min(32768,hard if hard!=resource.RLIM_INFINITY else 32768),hard))
 p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--endpoints',type=int,default=10);p.add_argument('--units',type=int,default=10);p.add_argument('--points',type=int,default=1000);p.add_argument('--clients',type=int,default=1);p.add_argument('--output');a=p.parse_args()
 if not 1<=a.clients<=10:p.error('本探测仅支持1～10客户端／端点')
 with tempfile.TemporaryDirectory(prefix='essim-m04-scale-') as tmp:
  root=Path(tmp);ports=[];reservations=[]
  for _ in range(a.endpoints):
   s=socket.socket();s.bind(('127.0.0.1',0));ports.append(s.getsockname()[1]);reservations.append(s)
  source=root/'snapshots.json';source.write_text(json.dumps([snapshot(f'e{i}',listen,a.units,a.points) for i,listen in enumerate(ports)]))
  for s in reservations:s.close()
  begin=time.monotonic();server=subprocess.Popen([a.binary,str(source),str(root/'runs')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);clients=[]
  try:
   for _ in ports:
    line=server.stdout.readline();assert line.startswith('{'),line+server.stderr.read()
   assert server.stdout.readline().strip()=='READY';startup=time.monotonic()-begin
   rss=int(subprocess.check_output(['ps','-o','rss=','-p',str(server.pid)],text=True).strip())
   threads=len(subprocess.check_output(['ps','-M','-p',str(server.pid)],text=True).splitlines())-1
   for listen in ports:
    group=[]
    for _ in range(a.clients):
     client=ModbusTcpClient('127.0.0.1',port=listen,timeout=10);assert client.connect();clients.append(client);group.append(client)
   start=time.monotonic();latencies=[];requests=0
   for client in clients:
    for unit in range(1,a.units+1):
     for address in range(0,a.points,125):
      t=time.monotonic();result=client.read_holding_registers(address,count=min(125,a.points-address),device_id=unit);latencies.append((time.monotonic()-t)*1000);assert not result.isError(),str(result);requests+=1
   duration=time.monotonic()-start
   rss_after=int(subprocess.check_output(['ps','-o','rss=','-p',str(server.pid)],text=True).strip())
   cpu=float(subprocess.check_output(["ps","-o","%cpu=","-p",str(server.pid)],text=True).strip())
   for client in clients:client.close()
   server.stdin.write('quit\n');server.stdin.flush();server.wait(timeout=60);assert server.returncode==0,server.stderr.read()
   report=dict(platform=platform.platform(),endpoints=a.endpoints,unitsPerEndpoint=a.units,pointsPerUnit=a.points,totalPoints=a.endpoints*a.units*a.points,clientsPerEndpoint=a.clients,connections=len(clients),startupSeconds=round(startup,3),scanDurationSeconds=round(duration,3),requests=requests,requestsPerSecond=round(requests/duration,2),latencyP50Ms=round(statistics.median(latencies),3),latencyP95Ms=round(sorted(latencies)[int(len(latencies)*.95)],3),rssAfterStartupKiB=rss,rssAfterScanKiB=rss_after,processThreads=threads,cpuPercentSnapshot=cpu,recording='sync_data per event',load='single sequential full scan per connected client; not sustained concurrent EMS load')
   text=json.dumps(report,ensure_ascii=False,indent=2);print(text)
   if a.output:Path(a.output).write_text(text+'\n')
  finally:
   for client in clients:client.close()
   if server.poll() is None:server.kill();server.wait()
if __name__=='__main__':main()
