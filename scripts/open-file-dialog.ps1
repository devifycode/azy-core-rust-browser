param([ValidateSet('Inspect','Cancel','Choose')][string]$Action='Inspect',[string]$File)
$ErrorActionPreference='Stop'
Add-Type -TypeDefinition @'
using System;using System.Text;using System.Collections.Generic;using System.Runtime.InteropServices;
public static class AzyOpenDialog {
 public delegate bool EnumProc(IntPtr hwnd,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr hwnd,EnumProc proc,IntPtr arg);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll")] static extern int GetDlgCtrlID(IntPtr hwnd);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder text,int length);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr hwnd,uint message,IntPtr wp,string text);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,EntryPoint="SendMessageW")] static extern IntPtr ReadMessage(IntPtr hwnd,uint message,IntPtr wp,StringBuilder text);
 public static string Text(IntPtr hwnd){var s=new StringBuilder(2048);ReadMessage(hwnd,0xD,new IntPtr(s.Capacity),s);return s.ToString();}
 static string Kind(IntPtr hwnd){var s=new StringBuilder(100);GetClassName(hwnd,s,100);return s.ToString();}
 public static IntPtr Root(uint process){IntPtr found=IntPtr.Zero;EnumWindows((hwnd,arg)=>{uint pid;GetWindowThreadProcessId(hwnd,out pid);if(pid==process&&Kind(hwnd)=="#32770"&&Text(hwnd).Contains("Buka file / PDF")){found=hwnd;return false;}return true;},IntPtr.Zero);return found;}
 public static List<object> Inspect(IntPtr root){var result=new List<object>();EnumChildWindows(root,(hwnd,arg)=>{string kind=Kind(hwnd);if(kind=="Edit"||kind=="Button"||kind=="ComboBox")result.Add(new{handle=hwnd.ToInt64(),id=GetDlgCtrlID(hwnd),kind=kind,text=Text(hwnd)});return true;},IntPtr.Zero);return result;}
 public static IntPtr Find(IntPtr root,int id,string kind,string text){IntPtr found=IntPtr.Zero;EnumChildWindows(root,(hwnd,arg)=>{if((id<0||GetDlgCtrlID(hwnd)==id)&&Kind(hwnd)==kind&&(text==null||Text(hwnd).Contains(text))){found=hwnd;return false;}return true;},IntPtr.Zero);return found;}
}
'@
$deadline=[DateTime]::UtcNow.AddSeconds(20)
do{$dialog=[AzyOpenDialog]::Root([uint32]$env:AZY_CORE_TEST_PID);if($dialog -ne [IntPtr]::Zero){break};Start-Sleep -Milliseconds 100}while([DateTime]::UtcNow -lt $deadline)
if($dialog -eq [IntPtr]::Zero){throw 'Open file dialog not found'}
if($Action -eq 'Inspect'){[AzyOpenDialog]::Inspect($dialog)|ConvertTo-Json -Depth 3 -Compress;exit}
if($Action -eq 'Cancel'){$button=[AzyOpenDialog]::Find($dialog,2,'Button',$null);[void][AzyOpenDialog]::SendMessage($button,0xF5,[IntPtr]::Zero,$null);exit}
$edit=[AzyOpenDialog]::Find($dialog,1148,'Edit',$null)
if($edit -eq [IntPtr]::Zero){throw 'Filename input not found'}
[void][AzyOpenDialog]::SendMessage($edit,0xC,[IntPtr]::Zero,$File)
$button=[AzyOpenDialog]::Find($dialog,1,'Button',$null);[void][AzyOpenDialog]::SendMessage($button,0xF5,[IntPtr]::Zero,$null)
