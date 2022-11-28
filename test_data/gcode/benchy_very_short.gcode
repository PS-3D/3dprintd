;FLAVOR:RepRap
;TIME:7793
;Filament used: 1.4491m
;Layer height: 0.1
;MINX:166.303
;MINY:184.064
;MINZ:0.3
;MAXX:229.789
;MAXY:215.935
;MAXZ:48
;Generated with Cura_SteamEngine 4.13.1
T0
M190 S60
M104 S200
M109 S200
M82 ;absolute extrusion mode
G28 ;Home
G1 Z15.0 F6000 ;Move the platform down 15mm
;Prime the extruder
G92 E0
G1 F200 E3
G92 E0
M83 ;relative extrusion mode
G1 F1500 E-6.5
;LAYER_COUNT:478
;LAYER:0
M107
G0 F3600 X169.749 Y187.713 Z0.3
;TYPE:SKIRT
G1 F1500 E6.5
G1 F1800 X169.936 Y187.562 E0.00452
G1 X170.843 Y186.931 E0.02078
G1 X171.041 Y186.813 E0.00434
G1 X171.939 Y186.353 E0.01898
G1 X172.131 Y186.27 E0.00393
G1 X173.101 Y185.925 E0.01937
G1 X173.373 Y185.848 E0.00532
G1 X174.143 Y185.672 E0.01486
G1 X174.802 Y185.58 E0.01252
G1 X175.275 Y185.537 E0.00893
;MESH:NONMESH
G0 F600 X175.5 Y194.457 Z0.5
G0 F5400 X200.867 Y207.555
G0 X200.877 Y207.728
;TIME_ELAPSED:185.408526
G1 F1500 E-6.5
M140 S0
M82 ;absolute extrusion mode
M107
M104 S0
M140 S0
;Retract the filament
G92 E1
G1 E-1 F300
G28 X0 Y0
M84
M83 ;relative extrusion mode
M104 S0
;End of Gcode
;SETTING_3 {"global_quality": "[general]\\nversion = 4\\nname = Fine #2\\ndefini
;SETTING_3 tion = custom\\n\\n[metadata]\\ntype = quality_changes\\nquality_type
;SETTING_3  = normal\\nsetting_version = 19\\n\\n[values]\\nadhesion_type = brim
;SETTING_3 \\n\\n", "extruder_quality": ["[general]\\nversion = 4\\nname = Fine 
;SETTING_3 #2\\ndefinition = fdmprinter\\n\\n[metadata]\\ntype = quality_changes
;SETTING_3 \\nquality_type = normal\\nsetting_version = 19\\nposition = 0\\n\\n[
;SETTING_3 values]\\n\\n"]}
