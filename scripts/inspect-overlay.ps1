$ErrorActionPreference='Stop'
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class AzyOverlayProbe {
 public delegate bool EnumProc(IntPtr hwnd,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr hwnd,EnumProc proc,IntPtr arg);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder name,int size);
 [DllImport("user32.dll")] static extern int GetWindowRgn(IntPtr hwnd,IntPtr region);
 [DllImport("user32.dll")] static extern uint GetDpiForWindow(IntPtr hwnd);
 [DllImport("gdi32.dll")] static extern IntPtr CreateRectRgn(int a,int b,int c,int d);
 [DllImport("gdi32.dll")] static extern bool PtInRegion(IntPtr region,int x,int y);
 [DllImport("gdi32.dll")] static extern bool DeleteObject(IntPtr obj);
 public static List<object> Inspect(IntPtr root,int x,int y,int width,int height) {
  var results=new List<object>();
  EnumChildWindows(root,(hwnd,arg)=>{
   var name=new StringBuilder(100);GetClassName(hwnd,name,100);
   if(name.ToString()=="WRY_WEBVIEW"){
    var r=CreateRectRgn(0,0,0,0);int kind=GetWindowRgn(hwnd,r);double scale=GetDpiForWindow(hwnd)/96.0;
    results.Add(new {kind=kind,toolbar=PtInRegion(r,(int)(8*scale),(int)(40*scale)),outside=PtInRegion(r,(int)(8*scale),(int)(110*scale)),dropdown=PtInRegion(r,(int)(250*scale),(int)(110*scale)),leftLeak=PtInRegion(r,(int)((x-1)*scale),(int)((y+24)*scale)),rightLeak=PtInRegion(r,(int)((x+width+1)*scale),(int)((y+24)*scale)),bottomLeak=PtInRegion(r,(int)((x+width/2)*scale),(int)((y+height+1)*scale)),corner=PtInRegion(r,(int)(x*scale),(int)(y*scale)),center=PtInRegion(r,(int)((x+width/2)*scale),(int)((y+height/2)*scale))});DeleteObject(r);
   }
   return true;
  },IntPtr.Zero);return results;
 }
}
'@
$process=Get-Process -Id ([int]$env:AZY_CORE_TEST_PID)
$bounds=if($env:AZY_OVERLAY_BOUNDS){$env:AZY_OVERLAY_BOUNDS | ConvertFrom-Json}else{@{x=0;y=0;width=0;height=0}}
[AzyOverlayProbe]::Inspect($process.MainWindowHandle,[int]$bounds.x,[int]$bounds.y,[int]$bounds.width,[int]$bounds.height) | ConvertTo-Json -Compress
