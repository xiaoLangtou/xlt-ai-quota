

获取待填写日报列表
fetch("[https://pms.prod.goktech.cn/gok/pms/dailyPaper/listMonthly](https://pms.prod.goktech.cn/gok/pms/dailyPaper/listMonthly)", {
"headers": {
"accept": "application/json, text/plain, /",
"accept-language": "zh-CN,zh;q=0.9",
"application": "1698521681078931458",
"authorization": "Bearer 80359fa4-aaf9-490d-9d8e-262cb4b79e59",
"content-type": "application/json",
"priority": "u=1, i",
"sec-ch-ua": ""Chromium";v="152", "Not?A_Brand";v="24", "Google Chrome";v="152"",
"sec-ch-ua-mobile": "?0",
"sec-ch-ua-platform": ""macOS"",
"sec-fetch-dest": "empty",
"sec-fetch-mode": "cors",
"sec-fetch-site": "same-origin",
"usertype": "2"
},
"referrer": "[https://pms.prod.goktech.cn/view-businesses/daily/write](https://pms.prod.goktech.cn/view-businesses/daily/write)",
"body": "{"startDate":"2026-09","approvalStatusList":[]}",
"method": "POST",
"mode": "cors",
"credentials": "include"
});


点击某一行日报编辑按钮，调用接口
fetch("[https://pms.prod.goktech.cn/gok/pms//holiday/getHolidayTypeByDate/2026-09-07](https://pms.prod.goktech.cn/gok/pms//holiday/getHolidayTypeByDate/2026-09-07)", {
"headers": {
"accept": "application/json, text/plain, /",
"accept-language": "zh-CN,zh;q=0.9",
"application": "1698521681078931458",
"authorization": "Bearer 80359fa4-aaf9-490d-9d8e-262cb4b79e59",
"priority": "u=1, i",
"sec-ch-ua": ""Chromium";v="152", "Not?A_Brand";v="24", "Google Chrome";v="152"",
"sec-ch-ua-mobile": "?0",
"sec-ch-ua-platform": ""macOS"",
"sec-fetch-dest": "empty",
"sec-fetch-mode": "cors",
"sec-fetch-site": "same-origin",
"usertype": "2"
},
"referrer": "[https://pms.prod.goktech.cn/view-businesses/daily/write](https://pms.prod.goktech.cn/view-businesses/daily/write)",
"body": null,
"method": "GET",
"mode": "cors",
"credentials": "include"
});

fetch("[https://pms.prod.goktech.cn/gok/pms/project/currentForDailyPaperEntryUnify?date=2026-09-07&collectType=1](https://pms.prod.goktech.cn/gok/pms/project/currentForDailyPaperEntryUnify?date=2026-09-07&collectType=1)", {
"headers": {
"accept": "application/json, text/plain, /",
"accept-language": "zh-CN,zh;q=0.9",
"application": "1698521681078931458",
"authorization": "Bearer 80359fa4-aaf9-490d-9d8e-262cb4b79e59",
"priority": "u=1, i",
"sec-ch-ua": ""Chromium";v="152", "Not?A_Brand";v="24", "Google Chrome";v="152"",
"sec-ch-ua-mobile": "?0",
"sec-ch-ua-platform": ""macOS"",
"sec-fetch-dest": "empty",
"sec-fetch-mode": "cors",
"sec-fetch-site": "same-origin",
"usertype": "2",
"cookie": "gok_auth=gok-auth-1b8914e5-c40d-4727-ab9e-a51ed9ca3a6e; token=80359fa4-aaf9-490d-9d8e-262cb4b79e59; refresh_token=f336b60a-f1f0-409b-93e6-8c8f10f713aa",
"Referer": "[https://pms.prod.goktech.cn/view-businesses/daily/write](https://pms.prod.goktech.cn/view-businesses/daily/write)"
},
"body": null,
"method": "GET"
});

{
    "code": 0,
    "msg": "操作成功",
    "data": [
        {
            "id": "812941",
            "projectName": "2025年信通人工智能专项业务运营项目",
            "isInsideProject": 2,
            "salesmanUserName": "覃发均",
            "managerUserName": "姜健",
            "tasks": [
                {
                    "id": "2013932098639843330",
                    "taskName": "项目管理成本挂靠",
                    "taskKind": 7,
                    "taskKindTxt": "成本挂靠",
                    "auditorNames": [
                        "曾裕梁"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": null,
                    "expectEndDate": null,
                    "type": 1,
                    "relateId": "2013932093124333570",
                    "workHourTypes": [
                        {
                            "name": "会议",
                            "value": 11
                        },
                        {
                            "name": "学习",
                            "value": 12
                        },
                        {
                            "name": "培训",
                            "value": 13
                        },
                        {
                            "name": "空转",
                            "value": 14
                        },
                        {
                            "name": "其他",
                            "value": 15
                        }
                    ]
                },
                {
                    "id": "2097972206807474177",
                    "taskName": "9.10-9.20知识库",
                    "taskKind": 2,
                    "taskKindTxt": "项目交付",
                    "auditorNames": [
                        "姜健"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": "2026-09-10",
                    "expectEndDate": "2026-09-20",
                    "type": 1,
                    "relateId": "2097972206400626690",
                    "workHourTypes": [
                        {
                            "name": "项目交付",
                            "value": 2
                        }
                    ]
                }
            ],
            "collectId": null,
            "collectUserId": null,
            "collectTime": null
        },
        {
            "id": "918941",
            "projectName": "教培知识库智能运维与培训取证管理一体化运营提升",
            "isInsideProject": 2,
            "salesmanUserName": "覃发均",
            "managerUserName": "姜健",
            "tasks": [
                {
                    "id": "2060317742728876034",
                    "taskName": "项目管理成本挂靠",
                    "taskKind": 7,
                    "taskKindTxt": "成本挂靠",
                    "auditorNames": [
                        "姜健"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": null,
                    "expectEndDate": null,
                    "type": 1,
                    "relateId": "2060317741260869634",
                    "workHourTypes": [
                        {
                            "name": "会议",
                            "value": 11
                        },
                        {
                            "name": "学习",
                            "value": 12
                        },
                        {
                            "name": "培训",
                            "value": 13
                        },
                        {
                            "name": "空转",
                            "value": 14
                        },
                        {
                            "name": "其他",
                            "value": 15
                        }
                    ]
                },
                {
                    "id": "2092415641450516487",
                    "taskName": "8.24-9.6取证",
                    "taskKind": 2,
                    "taskKindTxt": "项目交付",
                    "auditorNames": [
                        "姜健"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": "2026-08-24",
                    "expectEndDate": "2026-09-06",
                    "type": 1,
                    "relateId": "2092415635360387074",
                    "workHourTypes": [
                        {
                            "name": "项目交付",
                            "value": 2
                        }
                    ]
                },
                {
                    "id": "2096895645412941831",
                    "taskName": "9.7-9.20取证",
                    "taskKind": 2,
                    "taskKindTxt": "项目交付",
                    "auditorNames": [
                        "姜健"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": "2026-09-07",
                    "expectEndDate": "2026-09-20",
                    "type": 1,
                    "relateId": "2096895640912453633",
                    "workHourTypes": [
                        {
                            "name": "项目交付",
                            "value": 2
                        }
                    ]
                }
            ],
            "collectId": null,
            "collectUserId": null,
            "collectTime": null
        },
        {
            "id": "937941",
            "projectName": "27年组织部“AI+培训”项目",
            "isInsideProject": 2,
            "salesmanUserName": "覃发均",
            "managerUserName": "姜健",
            "tasks": [
                {
                    "id": "2047498286904733698",
                    "taskName": "项目管理成本挂靠",
                    "taskKind": 7,
                    "taskKindTxt": "成本挂靠",
                    "auditorNames": [
                        "曾裕梁"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": null,
                    "expectEndDate": null,
                    "type": 1,
                    "relateId": "2047498286766321666",
                    "workHourTypes": [
                        {
                            "name": "会议",
                            "value": 11
                        },
                        {
                            "name": "学习",
                            "value": 12
                        },
                        {
                            "name": "培训",
                            "value": 13
                        },
                        {
                            "name": "空转",
                            "value": 14
                        },
                        {
                            "name": "其他",
                            "value": 15
                        }
                    ]
                }
            ],
            "collectId": null,
            "collectUserId": null,
            "collectTime": null
        },
        {
            "id": "907441",
            "projectName": "2026年国网经营单元-数智培训业务部内部工时统计项目",
            "isInsideProject": 1,
            "salesmanUserName": "覃发均",
            "managerUserName": "覃发均",
            "tasks": [
                {
                    "id": "2013151942594813962",
                    "taskName": "非项目任务",
                    "taskKind": 16,
                    "taskKindTxt": "内部任务",
                    "auditorNames": [
                        "覃发均"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": "2026-01-01",
                    "expectEndDate": "2026-12-31",
                    "type": 0,
                    "relateId": null,
                    "workHourTypes": [
                        {
                            "name": "内部任务",
                            "value": 16
                        }
                    ]
                }
            ],
            "collectId": null,
            "collectUserId": null,
            "collectTime": null
        },
        {
            "id": "833442",
            "projectName": "2025-2026年国网信通数字化人才培育平台运营项目",
            "isInsideProject": 2,
            "salesmanUserName": "覃发均",
            "managerUserName": "陈民辉",
            "tasks": [
                {
                    "id": "2013932098639843398",
                    "taskName": "项目管理成本挂靠",
                    "taskKind": 7,
                    "taskKindTxt": "成本挂靠",
                    "auditorNames": [
                        "曾裕梁"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": null,
                    "expectEndDate": null,
                    "type": 1,
                    "relateId": "2013932094445539330",
                    "workHourTypes": [
                        {
                            "name": "会议",
                            "value": 11
                        },
                        {
                            "name": "学习",
                            "value": 12
                        },
                        {
                            "name": "培训",
                            "value": 13
                        },
                        {
                            "name": "空转",
                            "value": 14
                        },
                        {
                            "name": "其他",
                            "value": 15
                        }
                    ]
                }
            ],
            "collectId": null,
            "collectUserId": null,
            "collectTime": null
        },
        {
            "id": "783941",
            "projectName": "2025年软件系统运维中心内部管理工时统计项目",
            "isInsideProject": 1,
            "salesmanUserName": null,
            "managerUserName": "黄进强",
            "tasks": [
                {
                    "id": "1892520307646631937",
                    "taskName": "福建交付部空转",
                    "taskKind": 17,
                    "taskKindTxt": "成本挂靠",
                    "auditorNames": [
                        "曾裕梁"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": null,
                    "expectEndDate": null,
                    "type": 0,
                    "relateId": null,
                    "workHourTypes": [
                        {
                            "name": "会议",
                            "value": 18
                        },
                        {
                            "name": "学习",
                            "value": 19
                        },
                        {
                            "name": "空转",
                            "value": 20
                        },
                        {
                            "name": "培训",
                            "value": 21
                        },
                        {
                            "name": "其他",
                            "value": 22
                        }
                    ]
                }
            ],
            "collectId": null,
            "collectUserId": null,
            "collectTime": null
        },
        {
            "id": "551942",
            "projectName": "国网福建电力数字人才培育评价平台三期",
            "isInsideProject": 2,
            "salesmanUserName": "覃发均",
            "managerUserName": "陈民辉",
            "tasks": [
                {
                    "id": "2035566319084003329",
                    "taskName": "03.09-03.22评价活动改造",
                    "taskKind": 2,
                    "taskKindTxt": "项目交付",
                    "auditorNames": [
                        "陈向煜",
                        "曾裕梁"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": "2026-03-09",
                    "expectEndDate": "2026-03-22",
                    "type": 1,
                    "relateId": "2035566318928814082",
                    "workHourTypes": [
                        {
                            "name": "项目交付",
                            "value": 2
                        }
                    ]
                },
                {
                    "id": "2046906882700701705",
                    "taskName": "04.22-04.24评价改造",
                    "taskKind": 2,
                    "taskKindTxt": "项目交付",
                    "auditorNames": [
                        "陈向煜",
                        "曾裕梁"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": "2026-04-22",
                    "expectEndDate": "2026-04-24",
                    "type": 1,
                    "relateId": "2046906881941532673",
                    "workHourTypes": [
                        {
                            "name": "项目交付",
                            "value": 2
                        }
                    ]
                },
                {
                    "id": "2062823550778290185",
                    "taskName": "6.4-6.18评价活动考试改造",
                    "taskKind": 2,
                    "taskKindTxt": "项目交付",
                    "auditorNames": [
                        "姜健",
                        "陈向煜"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": "2026-06-04",
                    "expectEndDate": "2026-06-17",
                    "type": 1,
                    "relateId": "2062823550316916737",
                    "workHourTypes": [
                        {
                            "name": "项目交付",
                            "value": 2
                        }
                    ]
                },
                {
                    "id": "2066719070710464517",
                    "taskName": "6.15-6.28(3311功能开发)",
                    "taskKind": 2,
                    "taskKindTxt": "项目交付",
                    "auditorNames": [
                        "姜健"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": "2026-06-15",
                    "expectEndDate": "2026-06-28",
                    "type": 1,
                    "relateId": "2066719070567858177",
                    "workHourTypes": [
                        {
                            "name": "项目交付",
                            "value": 2
                        }
                    ]
                }
            ],
            "collectId": null,
            "collectUserId": null,
            "collectTime": null
        },
        {
            "id": "443441",
            "projectName": "国网福建电力数字人才培育评价平台二期",
            "isInsideProject": 2,
            "salesmanUserName": "陈镇斌",
            "managerUserName": "卢金灵",
            "tasks": [
                {
                    "id": "1765349891928326145",
                    "taskName": "交付工作",
                    "taskKind": 3,
                    "taskKindTxt": "解决方案支撑",
                    "auditorNames": [
                        "卢金灵"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": null,
                    "expectEndDate": null,
                    "type": 0,
                    "relateId": null,
                    "workHourTypes": [
                        {
                            "name": "解决方案支撑",
                            "value": 3
                        }
                    ]
                }
            ],
            "collectId": null,
            "collectUserId": null,
            "collectTime": null
        },
        {
            "id": "37441",
            "projectName": "泉州职业技术大学华为ICT学院",
            "isInsideProject": 2,
            "salesmanUserName": "叶龙",
            "managerUserName": "叶龙",
            "tasks": [
                {
                    "id": "1765348641665024002",
                    "taskName": "其他任务",
                    "taskKind": 2,
                    "taskKindTxt": "项目交付",
                    "auditorNames": [
                        "王雅婷",
                        "郑婉甜"
                    ],
                    "taskFinishFlag": false,
                    "expectStartDate": null,
                    "expectEndDate": null,
                    "type": 0,
                    "relateId": null,
                    "workHourTypes": [
                        {
                            "name": "项目交付",
                            "value": 2
                        }
                    ]
                }
            ],
            "collectId": null,
            "collectUserId": null,
            "collectTime": null
        }
    ]
}


fetch("[https://pms.prod.goktech.cn/gok/pms/dailyPaper/details/2097203282214051845](https://pms.prod.goktech.cn/gok/pms/dailyPaper/details/2097203282214051845)", {
"headers": {
"accept": "application/json, text/plain, /",
"accept-language": "zh-CN,zh;q=0.9",
"application": "1698521681078931458",
"authorization": "Bearer 80359fa4-aaf9-490d-9d8e-26tttthyh2cb4b79e59",
"priority": "u=1, i",
"sec-ch-ua": ""Chromium";v="152", "Not?A_Brand";v="24", "Google Chrome";v="152"",
"sec-ch-ua-mobile": "?0",
"sec-ch-ua-platform": ""macOS"",
"sec-fetch-dest": "empty",
"sec-fetch-mode": "cors",
"sec-fetch-site": "same-origin",
"usertype": "2"
},
"referrer": "[https://pms.prod.goktech.cn/view-businesses/daily/write](https://pms.prod.goktech.cn/view-businesses/daily/write)",
"body": null,
"method": "GET",
"mode": "cors",
"credentials": "include"
});

{
    "code": 0,
    "msg": "操作成功",
    "data": {
        "dailyPaper": {
            "id": "2097203282214051845",
            "userRealName": "魏鹏程",
            "userStatus": 0,
            "userStatusName": "正式",
            "submissionDate": "2026-09-07",
            "submissionDateFormatted": "2026-09-07（周一）",
            "ctimeFormatted": "",
            "ctime": "2026-09-08 14:00:01",
            "mtime": "2026-09-08 14:00:01",
            "mtimeDate": "2026-09-08",
            "workday": 1,
            "workdayName": "无日报",
            "projectCount": 0,
            "taskCount": 0,
            "dailyHourCount": 0.0,
            "compensatoryHourCount": 0,
            "addedHourCount": 0.0,
            "approvalStatus": -1,
            "approvalStatusStrings": null,
            "approvalStatusNameList": null,
            "approvalStatusName": "未填报",
            "hourData": null,
            "leaveHourData": 0,
            "leaveHours": 0,
            "ifFiled": false,
            "ifHoliday": null,
            "holidayType": null,
            "ifAbnormal": null,
            "abnormalMsg": null,
            "totalHour": 0.0,
            "submissionTime": null
        },
        "abnormalDesc": "日报未提交",
        "entries": [],
        "tomorrowPlanPaperEntries": []
    }
}


保存接口
fetch("https://pms.prod.goktech.cn/gok/pms/dailyPaper/addOrUpdate", {
  "headers": {
    "accept": "application/json, text/plain, */*",
    "accept-language": "zh-CN,zh;q=0.9",
    "application": "1698521681078931458",
    "authorization": "Bearer 80359fa4-aaf9-490d-9d8e-262cb4b79e59",
    "content-type": "application/json",
    "priority": "u=1, i",
    "sec-ch-ua": "\"Chromium\";v=\"152\", \"Not?A_Brand\";v=\"24\", \"Google Chrome\";v=\"152\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"macOS\"",
    "sec-fetch-dest": "empty",
    "sec-fetch-mode": "cors",
    "sec-fetch-site": "same-origin",
    "usertype": "2",
    "cookie": "gok_auth=gok-auth-1b8914e5-c40d-4727-ab9e-a51ed9ca3a6e; token=80359fa4-aaf9-490d-9d8e-262cb4b79e59; refresh_token=f336b60a-f1f0-409b-93e6-8c8f10f713aa",
    "Referer": "https://pms.prod.goktech.cn/view-businesses/daily/write"
  },
  "body": "{\"id\":\"2097203282214051845\",\"submissionDate\":\"2026-09-07\",\"submit\":true,\"entries\":[{\"projectId\":\"918941\",\"projectName\":\"教培知识库智能运维与培训取证管理一体化运营提升\",\"taskId\":\"2096895645412941831\",\"taskName\":\"8.24-9.6取证\",\"dailyPaperId\":\"2096116122523635713\",\"approvalStatus\":0,\"submissionDate\":\"2026-09-04\",\"workType\":2,\"workTypeTxt\":\"项目交付\",\"description\":\"1. 修复试卷信息条件渲染错误  \\n2. 标记已停止的证书维度列  \\n3. 优化证据管理按钮及代码格式\",\"approvalReason\":\"\",\"userId\":\"953\",\"userRealName\":\"魏鹏程\",\"userDeptId\":\"2012079058773045250\",\"isInsideProject\":2,\"taskFinishFlag\":false,\"workHourTypes\":[{\"name\":\"项目交付\",\"value\":2}],\"taskOptions\":[{\"id\":\"2060317742728876034\",\"taskName\":\"项目管理成本挂靠\",\"taskKind\":7,\"taskKindTxt\":\"成本挂靠\",\"auditorNames\":[\"姜健\"],\"taskFinishFlag\":false,\"expectStartDate\":null,\"expectEndDate\":null,\"type\":1,\"relateId\":\"2060317741260869634\",\"workHourTypes\":[{\"name\":\"会议\",\"value\":11},{\"name\":\"学习\",\"value\":12},{\"name\":\"培训\",\"value\":13},{\"name\":\"空转\",\"value\":14},{\"name\":\"其他\",\"value\":15}]},{\"id\":\"2092415641450516487\",\"taskName\":\"8.24-9.6取证\",\"taskKind\":2,\"taskKindTxt\":\"项目交付\",\"auditorNames\":[\"姜健\"],\"taskFinishFlag\":false,\"expectStartDate\":\"2026-08-24\",\"expectEndDate\":\"2026-09-06\",\"type\":1,\"relateId\":\"2092415635360387074\",\"workHourTypes\":[{\"name\":\"项目交付\",\"value\":2}]},{\"id\":\"2096895645412941831\",\"taskName\":\"9.7-9.20取证\",\"taskKind\":2,\"taskKindTxt\":\"项目交付\",\"auditorNames\":[\"姜健\"],\"taskFinishFlag\":false,\"expectStartDate\":\"2026-09-07\",\"expectEndDate\":\"2026-09-20\",\"type\":1,\"relateId\":\"2096895640912453633\",\"workHourTypes\":[{\"name\":\"项目交付\",\"value\":2}]}],\"addedHours\":0,\"normalHours\":7,\"approvalStatusName\":\"未提交\",\"overtimeFiles\":\"\"}],\"tomorrowPlanPaperEntries\":[{\"projectId\":\"918941\",\"projectName\":\"教培知识库智能运维与培训取证管理一体化运营提升\",\"taskId\":\"2096895645412941831\",\"taskName\":\"8.24-9.6取证\",\"dailyPaperId\":\"2096116122523635713\",\"approvalStatus\":0,\"submissionDate\":\"2026-09-04\",\"workType\":2,\"workTypeTxt\":\"项目交付\",\"description\":\"需求代码编写\",\"approvalReason\":\"\",\"userId\":\"953\",\"userRealName\":\"魏鹏程\",\"userDeptId\":\"2012079058773045250\",\"isInsideProject\":2,\"taskFinishFlag\":false,\"workHourTypes\":[{\"name\":\"项目交付\",\"value\":2}],\"taskOptions\":[{\"id\":\"2060317742728876034\",\"taskName\":\"项目管理成本挂靠\",\"taskKind\":7,\"taskKindTxt\":\"成本挂靠\",\"auditorNames\":[\"姜健\"],\"taskFinishFlag\":false,\"expectStartDate\":null,\"expectEndDate\":null,\"type\":1,\"relateId\":\"2060317741260869634\",\"workHourTypes\":[{\"name\":\"会议\",\"value\":11},{\"name\":\"学习\",\"value\":12},{\"name\":\"培训\",\"value\":13},{\"name\":\"空转\",\"value\":14},{\"name\":\"其他\",\"value\":15}]},{\"id\":\"2092415641450516487\",\"taskName\":\"8.24-9.6取证\",\"taskKind\":2,\"taskKindTxt\":\"项目交付\",\"auditorNames\":[\"姜健\"],\"taskFinishFlag\":false,\"expectStartDate\":\"2026-08-24\",\"expectEndDate\":\"2026-09-06\",\"type\":1,\"relateId\":\"2092415635360387074\",\"workHourTypes\":[{\"name\":\"项目交付\",\"value\":2}]},{\"id\":\"2096895645412941831\",\"taskName\":\"9.7-9.20取证\",\"taskKind\":2,\"taskKindTxt\":\"项目交付\",\"auditorNames\":[\"姜健\"],\"taskFinishFlag\":false,\"expectStartDate\":\"2026-09-07\",\"expectEndDate\":\"2026-09-20\",\"type\":1,\"relateId\":\"2096895640912453633\",\"workHourTypes\":[{\"name\":\"项目交付\",\"value\":2}]}],\"approvalStatusName\":\"未提交\"}]}",
  "method": "POST"
});

需要人为选择的是:
1、登录的token, pms为单点登录，https://gok-cas-prod-http.prod.goktech.cn/cas/login?service=https://pms.prod.goktech.cn/gok/pms  需要我输入单点登录账号密码
fetch("https://gok-cas-prod-http.prod.goktech.cn/cas/login?service=https://pms.prod.goktech.cn/gok/pms", {
  "headers": {
    "accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
    "accept-language": "zh-CN,zh;q=0.9",
    "cache-control": "max-age=0",
    "content-type": "application/x-www-form-urlencoded",
    "priority": "u=0, i",
    "sec-ch-ua": "\"Chromium\";v=\"152\", \"Not?A_Brand\";v=\"24\", \"Google Chrome\";v=\"152\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"macOS\"",
    "sec-fetch-dest": "document",
    "sec-fetch-mode": "navigate",
    "sec-fetch-site": "same-origin",
    "sec-fetch-user": "?1",
    "upgrade-insecure-requests": "1"
  },
  "body": "uname=weipc&passwd=%21Wpc12345678&phone=&phoneCode=&execution=e2s1&_eventId=submit&geolocation=&loginType=1",
  "method": "POST",
  "mode": "cors",
  "credentials": "include"
});

fetch("https://pms.prod.goktech.cn/gok/pms?ticket=ST-39813--UCs33aMKdl0ADul5AdumRAclHggok-cas-server-684c69f68c-wsrkm", {
  "headers": {
    "accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
    "accept-language": "zh-CN,zh;q=0.9",
    "cache-control": "max-age=0",
    "priority": "u=0, i",
    "sec-ch-ua": "\"Chromium\";v=\"152\", \"Not?A_Brand\";v=\"24\", \"Google Chrome\";v=\"152\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"macOS\"",
    "sec-fetch-dest": "document",
    "sec-fetch-mode": "navigate",
    "sec-fetch-site": "same-site",
    "sec-fetch-user": "?1",
    "upgrade-insecure-requests": "1",
    "cookie": "gok_auth=gok-auth-1b8914e5-c40d-4727-ab9e-a51ed9ca3a6e"
  },
  "body": null,
  "method": "GET"
});

fetch("https://pms.prod.goktech.cn/gok/auth/oauth/token?grant_type=password", {
  "headers": {
    "accept": "application/json, text/plain, */*",
    "accept-language": "zh-CN,zh;q=0.9",
    "application": "1698521681078931458",
    "authorization": "Basic Y2FzOmNhcw==",
    "content-type": "application/x-www-form-urlencoded",
    "noauth": "1",
    "priority": "u=1, i",
    "sec-ch-ua": "\"Chromium\";v=\"152\", \"Not?A_Brand\";v=\"24\", \"Google Chrome\";v=\"152\"",
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": "\"macOS\"",
    "sec-fetch-dest": "empty",
    "sec-fetch-mode": "cors",
    "sec-fetch-site": "same-origin",
    "usertype": "2",
    "cookie": "gok_auth=gok-auth-1b8914e5-c40d-4727-ab9e-a51ed9ca3a6e",
    "Referer": "https://pms.prod.goktech.cn/"
  },
  "body": "password=",
  "method": "POST"
});

{
    "access_token": "378e03fd-e3eeeftttt34-4cf1-a995-f68808719b8911",
    "token_type": "bearer",
    "refresh_token": "a1449ca7-63f3-40fb-tttt8638ttt-8d1b21402226a63",
    "expires_in": 99999999,
    "scope": "server",
    "license": "made by gok-base",
    "active": true,
    "user_id": "953",
    "client_id": "cas",
    "username": "weipc"
}

2、固定的：正常工时：7
3、需要手动确认的是：项目、工单/任务、加班工时
