param([ValidateSet('left','right')][string]$Pane)
$ErrorActionPreference='Stop'
# Test-only native focus probe, restricted to the isolated browser process.
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class AzyPaneFocus {
 public delegate bool EnumProc(IntPtr hwnd,IntPtr arg);
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr hwnd,EnumProc proc,IntPtr arg);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder value,int length);
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hwnd,out Rect rect);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
 [DllImport("user32.dll")] static extern bool AttachThreadInput(uint one,uint two,bool attach);
 [DllImport("user32.dll")] static extern IntPtr SetFocus(IntPtr hwnd);
 [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr hwnd);
 public static bool Focus(IntPtr root,bool right) {
  IntPtr target=IntPtr.Zero;int targetX=right?int.MinValue:int.MaxValue;Rect rootRect;GetWindowRect(root,out rootRect);
  EnumChildWindows(root,(hwnd,arg)=>{var name=new StringBuilder(100);GetClassName(hwnd,name,100);Rect r;
   if(name.ToString()=="Chrome_WidgetWin_0"&&IsWindowVisible(hwnd)&&GetWindowRect(hwnd,out r)&&r.Bottom-r.Top>100&&r.Right-r.Left>100&&r.Right-r.Left<rootRect.Right-rootRect.Left-100){
    if((right&&r.Left>targetX)||(!right&&r.Left<targetX)){target=hwnd;targetX=r.Left;}
   }return true;
  },IntPtr.Zero);
  if(target==IntPtr.Zero)return false;
  uint pid;uint other=GetWindowThreadProcessId(target,out pid),own=GetCurrentThreadId();
  AttachThreadInput(own,other,true);
  try{SetForegroundWindow(root);SetFocus(target);}finally{AttachThreadInput(own,other,false);}return true;
 }
}
'@
$process=Get-Process -Id ([int]$env:AZY_CORE_TEST_PID)
if(-not [AzyPaneFocus]::Focus($process.MainWindowHandle,($Pane -eq 'right'))){throw 'Native test pane not found'}
