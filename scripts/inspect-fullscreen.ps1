param([ValidateSet('Inspect','Activate','SwitchAway')][string]$Action='Inspect')
$ErrorActionPreference='Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class AzyFullscreenProbe {
 public delegate bool EnumProc(IntPtr hwnd,IntPtr arg);
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int left,top,right,bottom; }
 [StructLayout(LayoutKind.Sequential)] public struct Point { public int x,y; }
 [StructLayout(LayoutKind.Sequential)] public struct Monitor { public int size; public Rect bounds,work; public uint flags; }
 [DllImport("user32.dll")] static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr h,ref Point p);
 [DllImport("user32.dll")] static extern IntPtr MonitorFromWindow(IntPtr h,uint flags);
 [DllImport("user32.dll")] static extern bool GetMonitorInfo(IntPtr h,ref Monitor m);
 [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(Point p);
 [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr h,uint flags);
 [DllImport("user32.dll")] static extern long GetWindowLongPtr(IntPtr h,int index);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr h,EnumProc callback,IntPtr arg);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr h,StringBuilder name,int size);
 [DllImport("user32.dll")] static extern int GetWindowRgn(IntPtr h,IntPtr r);
 [DllImport("gdi32.dll")] static extern IntPtr CreateRectRgn(int a,int b,int c,int d);
 [DllImport("gdi32.dll")] static extern bool PtInRegion(IntPtr r,int x,int y);
 [DllImport("gdi32.dll")] static extern bool DeleteObject(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern IntPtr CreateWindowEx(uint ex,string cls,string name,uint style,int x,int y,int w,int h,IntPtr parent,IntPtr menu,IntPtr instance,IntPtr param);
 [DllImport("user32.dll")] static extern bool DestroyWindow(IntPtr h);
 public static object Inspect(IntPtr root,string action) {
  SetProcessDPIAware();
  if(action=="Activate") { SetForegroundWindow(root); System.Threading.Thread.Sleep(250); }
  IntPtr other=IntPtr.Zero;
  try {
   if(action=="SwitchAway") { other=CreateWindowEx(0,"STATIC","Azy fullscreen focus test",0x90000000,0,0,160,80,IntPtr.Zero,IntPtr.Zero,IntPtr.Zero,IntPtr.Zero); SetForegroundWindow(other); System.Threading.Thread.Sleep(350); }
   Rect r; GetWindowRect(root,out r);var m=new Monitor();m.size=Marshal.SizeOf(m);GetMonitorInfo(MonitorFromWindow(root,2),ref m);
   var origin=new Point();ClientToScreen(root,ref origin);bool toolbar=false;var children=new System.Collections.Generic.List<object>();
   EnumChildWindows(root,(h,a)=>{var cls=new StringBuilder(100);GetClassName(h,cls,100);if(cls.ToString()=="WRY_WEBVIEW") {var region=CreateRectRgn(0,0,0,0);Rect child;GetWindowRect(h,out child);int kind=GetWindowRgn(h,region);children.Add(new{child,kind,visible=IsWindowVisible(h)});if(IsWindowVisible(h) && child.top==origin.y && ((kind!=0 && PtInRegion(region,8,40)) || (kind==0 && child.bottom-child.top<=160)))toolbar=true;DeleteObject(region);}return true;},IntPtr.Zero);
   var p=new Point{x=(m.bounds.left+m.bounds.right)/2,y=m.bounds.bottom-2};
   return new {monitorCovered=r.left==m.bounds.left&&r.top==m.bounds.top&&r.right==m.bounds.right&&r.bottom==m.bounds.bottom,taskbarAreaOwned=GetAncestor(WindowFromPoint(p),2)==root,foreground=GetForegroundWindow()==root,topmost=(GetWindowLongPtr(root,-20)&8)!=0,toolbarVisible=toolbar,children=children,window=r,monitor=m.bounds};
  } finally { if(other!=IntPtr.Zero){SetForegroundWindow(root);DestroyWindow(other);} }
 }
}
'@
$process=Get-Process -Id ([int]$env:AZY_CORE_TEST_PID)
[AzyFullscreenProbe]::Inspect($process.MainWindowHandle,$Action) | ConvertTo-Json -Depth 4 -Compress
