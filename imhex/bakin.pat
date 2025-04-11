#pragma once

import std.string;
import std.core;
import type.leb128;
     
namespace auto bakin{
using SizedString = std::string::SizedStringBase<type::LEB128,char>;
using NullString = std::string::NullString;
using Event;
using EventSheet;
using EventType;
using Condition;
using EventSheetCondition;
using VariableName;
using Section;

    struct RbrFile{
        if(std::mem::read_unsigned($,5,std::mem::Endian::Big) != 0x59554B4152){
            return;
        }
        u8 signature [5];
        u32 header_length;
        u16;
        u8 header[header_length];
        Section sections[while(!std::mem::eof())];
        // Section sections[1];
    };

    enum SectionType : u16{
        GameSetting = 0x0001,
        EntityHeader = 0x0007,
        EventSheet = 0x1007,
        CameraSetting = 0x4003,//to be confirmed
        MapData = 0x0003,//to be confirmed
        JobData = 0x000B,
        ItemData = 0x000C,
        SkillData = 0x000D,
    };

    struct Section{
        u32 length [[color("FFFF00")]];
        be SectionType section_type [[color("00FF00")]];
        u32 begin_section = $;
        u32 data[4];
        match(section_type){ 
            (SectionType::GameSetting):{
                SizedString game_name;
              
            }
            (SectionType::EntityHeader):{
                SizedString entity_name;
                u8 _padding[2];
                u32 event_sheet_num;
                EventSheetCondition event_sheet_conditions[event_sheet_num] [[single_color ,color("808080")]];
                u128;
                SizedString text;
                u128;
                u32;
                SizedString script_name;
                SizedString text2;
                u8 header_end[7];
            }
            (SectionType::EventSheet):{
                SizedString value;
                u8 _padding[2];
                u32 event_sheet_num;
                //EventSheet event_sheets[1];
                u32 event_num [[color("00FF00")]];
                Event events[event_num] [[single_color ,color("808080")]];
                u8 event_sheet_end[2] [[color("FF0000")]];
            }
            (SectionType::CameraSetting):{
                SizedString;
                SizedString;
            }
            (SectionType::JobData):{
                SizedString name;
            }
            (SectionType::ItemData):{
                SizedString name;
                SizedString note;
                u128;
                u8;
                SizedString description;
                u32;
            }
            (SectionType::SkillData):{
                SizedString name;
                u16;
                u128;
                SizedString description;

            }
        } 
        u32 current_cursor = $;
        u8 end[length - (current_cursor - begin_section)] [[color("FF0000")]];
    };


    #define SKIP_EVENT_SHEET_CONDITION false
    struct EventSheetCondition{
        u32 length;
        if(SKIP_EVENT_SHEET_CONDITION){
            u8 data[length];
        }else{
            u32 condition_num;
            Condition conditions[condition_num];
            SizedString event_sheet_name;
            u8;
            u32 a[3];
            
            u32 orientation;
            u32 a;
            u32 collide_with_player;
            s32 movement_speed;
            u8 fixed_oreintation,movement_pattern;
            u8 b[3];
            u32 movement_frequency;
            u32 c[4];
            VariableName motion_name;
            be u32 limit_movement_range;
            u32 right,left,top,bottom;
            u8;
        
            u32 custom_collide;
            float collide_x, collide_y, collide_z;
            u8 d[14];
            u32 map_image;
        }
    };

    enum VariableScope:u8{
        Value = 0x00,
        Local = 0x01,
        Array = 0x02,
        CrossSave = 0x03,
        GraphicMotion = 0x11,//not sure
    };

    struct VariableName{
        SizedString variable_name;
        VariableScope variable_scope;
    };

    enum ConditionType:u32{
        EventSwitch = 0x00,
        VariableBox = 0x01,
        Money = 0x02,
        Item = 0x03,
        ItemEquipped = 0x05,
        Party = 0x04,
        Coordinate = 0x07,
        Collision = 0x08,
        
        EventSwitchArray = 0x0A,
        ArrayVariableBox = 0x0B,
    };

    enum CompareMethod:u32{
        Equals = 0x00,// Coordinate inside camera
        NotEquals = 0x01,// Coordinate outside camera
        GreaterOrEquals = 0x02,
        LesserOrEquals = 0x03,
        GreaterThan = 0x04,
        LessThan = 0x05,
    };

    struct Condition{
        le ConditionType condition_type;
        CompareMethod compare_method;
        u32 collision_target,value;
        
        u32 reference[4];
        VariableName variable_name;
        match(condition_type){
            (ConditionType::EventSwitchArray | ConditionType::ArrayVariableBox):{
                u32 variable;
                if(variable == 0xFFFFFFFF){
                    SizedString variable_name;
                }else{
                    u8;
                }
            }
        }
    };


    struct EventSheet{
        u32 event_num [[color("00FF00")]];
        Event events[event_num] [[single_color ,color("808080")]];
        u8 end[2] [[color("FF0000")]];
    };


    struct EventDataType<auto value>{
        match(value){
            (0x01):u32;
            (0x02):u128;
            (0x03):SizedString text;
            (0x04):SizedString variable_name;
            (0x05):SizedString switch_name;
            (0x06):{ // to be confirmed
                u128; 
                u8 a[5];
                float x,y,z;
            }
            (0x07):{
                SizedString array_name;
                u32 type;
                match (type){
                    (0x01):u32;
                    (0x02):u128;
                    (0x03):SizedString text;
                    (0x04):SizedString variable_name;
                    (0x05):SizedString switch_name;
                    (0x08):u32; // to be confirmed
                    (_):u32;
                }
            }
            (0x08):float;
        }
    };

    struct Event{
        le EventType event_type [[color("0000FF")]];
        u32 nest_depth;
        u8 data[while($[$] != 0x00)];
        u8 end;
        EventDataType<data[std::core::array_index()]> variables[std::core::member_count(data)];

    };

    enum EventType:u32 {
        Conversation = 0x2B,
        Message = 0x1D,
        TickerText = 0x2C,
        Emoticon = 0x2D,
        ChangeExpression = 0x82,
        AddToBackLog = 0xB6,
        
        TeleportPlayer = 0x14,
        PlaySound = 0x16,
        DisplayImage = 0x19,
        DisplayTextAsImage = 0x1A,
        DisplayEffect= 0x1B,


        ChangeScreenBrightness = 0x3C,
        ChangeItem = 0x21,
        ChangeMoney = 0x22,
        ChangeVariable = 0x0F,
        CheckVariable = 0x2F,
        CheckItem = 0x30,
        CheckMoney = 0x31,
        
        Note = 0x7E,
        Select = 0x1E,
        SelectSeparator = 0x4A,
        Brancing = 0x47,
        EndBranching = 0x48,

        AssignString = 0x53,
        CallCommonEvent = 0x05,
        CallCShrapProgram = 0xA4,
        CommentOut = 0xA9,
    };
    




    struct DisplayPosition<auto type>{
        if(type == 0x01){ // normal bubble
            u8 display_position; //00top 01center 02bottom if bubble 00-08 is cast number
            u8 is_bubble; // 0x10 == bubble
            u8 a[6];
        }else{ //event bubble
            u8 entity_ref[20];
        }
    };





}
