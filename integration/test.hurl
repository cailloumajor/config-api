GET http://config-api:8080/config/unknownCollection/unknownId

HTTP 404
[Asserts]
body contains "unknownCollection"
body contains "unknownId"


GET http://config-api:8080/config/firstCollection/one

HTTP 200
[Asserts]
jsonpath "$.first" == false
jsonpath "$.second" == 1


GET http://config-api:8080/config/firstCollection/two

HTTP 200
[Asserts]
jsonpath "$.first" == true
jsonpath "$.second" == 2


GET http://config-api:8080/config/secondCollection/one

HTTP 200
[Asserts]
jsonpath "$.some" == "value"
jsonpath "$.other" == 37.5


GET http://config-api:8080/config/secondCollection/two

HTTP 200
[Asserts]
jsonpath "$.some" == "otherVal"
jsonpath "$.other" == 42.9